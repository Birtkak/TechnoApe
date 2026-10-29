# --- the worker queue is a priority heap (ADR 0108) ---
# Part of the TechnoApe godot_voxel patches: run in any order.
# ThreadedTaskRunner sorted its queue every 200 ms and took tasks from the back, so between two sorts it ran the
# newest tasks first: in flight those are the far edge of the box the update task just requested, and the ground
# under the player waited (3 s into a 100 m flight, 3 k of its data blocks were still loading behind a 178 k queue).
# The queue is now a max-heap on cached priority: a staged task gets its priority when it enters (outside the staging
# lock) and is pushed at its place, a worker pops the best one, and the 200 ms refresh re-evaluates every priority
# and rebuilds the heap (O(n), cheaper than the sort it replaces). A serial task that cannot run yet is set aside and
# pushed back after the pick.
# PriorityDependency::evaluate also started its closest-viewer search at 99999 squared units: with one unit = one
# 12.5 cm voxel that is 39.5 m, so every block farther than that had the same priority (all of a 100 m box's
# generation ran in arbitrary order). It starts at the float maximum now.
# Lowering the thread count joined every worker and restarted them, waiting out whatever each ran (50-190 ms when the
# loading cover lifted on a 180 k backlog). Lowering it now only parks the workers over the count (they stop picking
# tasks, sleep 2 ms at a time and count as waiting); only raising it past the threads that exist restarts them.
import sys
root = sys.argv[1]
REVERT = len(sys.argv) > 2 and sys.argv[2] == 'revert'

def patch(rel, old, new):
    if REVERT:
        old, new = new, old
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

C = 'util/tasks/threaded_task_runner.cpp'
patch(C, '''#include "../string/format.h"
''', '''#include "../string/format.h"
#include <algorithm> // TechnoApe ADR 0108
''')

patch(C, '''				// Move tasks from the staging queue.
				// Lock with minimal risk of blocking the main thread, it should be very short.
				if (_staged_tasks_mutex.try_lock()) {
					append_array(_tasks, _staged_tasks);
					_staged_tasks.clear();
					_staged_tasks_mutex.unlock();
				}
''', '''				// TechnoApe ADR 0108: _tasks is a max-heap on cached_priority
				struct HeapLess {
					inline bool operator()(const TaskItem &a, const TaskItem &b) const {
						return a.cached_priority < b.cached_priority;
					}
				};

				// Move tasks from the staging queue.
				// Lock with minimal risk of blocking the main thread, it should be very short.
				static thread_local StdVector<TaskItem> tls_staged; // TechnoApe ADR 0108
				if (_staged_tasks_mutex.try_lock()) {
					std::swap(tls_staged, _staged_tasks);
					_staged_tasks_mutex.unlock();
				}
				for (TaskItem &item : tls_staged) {
					item.cached_priority = item.task->get_priority();
					_tasks.push_back(item);
					std::push_heap(_tasks.begin(), _tasks.end(), HeapLess());
				}
				tls_staged.clear();
''')

patch(C, '''						struct TaskComparator {
							inline bool operator()(const TaskItem &a, const TaskItem &b) const {
								// Tasks with highest priority come last (easier pop back)
								return a.cached_priority < b.cached_priority;
							}
						};
						SortArray<TaskItem, TaskComparator> sorter;
						sorter.sort(_tasks.data(), _tasks.size());
''', '''						std::make_heap(_tasks.begin(), _tasks.end(), HeapLess()); // TechnoApe ADR 0108
''')

patch(C, '''					// Pick task with highest priority if possible
					// for (int i = int(_tasks.size()) - 1; i >= 0; --i) {
					for (unsigned int i = _tasks.size(); i-- > 0;) {
						const TaskItem item = _tasks[i];
						// Serial tasks are a bit annoying in that regard...
						// We could make the save/load tasks accept more than one work, which is the best way to do
						// serial work, but in some cases it's harder to know in advance...
						if (item.is_serial && _is_serial_task_running) {
							// Try previous task
							continue;
						}

						tasks.push_back(item);
						// We don't just pop the last item because of serial task handling. But ordered removal should
						// be fast enough since serial tasks aren't common.
						_tasks.erase(_tasks.begin() + i);
						break;
					}
''', '''					// Pick task with highest priority if possible
					// TechnoApe ADR 0108: pop the heap; serial tasks that cannot run yet go back after the pick
					static thread_local StdVector<TaskItem> tls_held;
					while (_tasks.size() > 0) {
						std::pop_heap(_tasks.begin(), _tasks.end(), HeapLess());
						const TaskItem item = _tasks.back();
						_tasks.pop_back();
						if (item.is_serial && _is_serial_task_running) {
							tls_held.push_back(item);
							continue;
						}
						tasks.push_back(item);
						break;
					}
					for (const TaskItem &item : tls_held) {
						_tasks.push_back(item);
						std::push_heap(_tasks.begin(), _tasks.end(), HeapLess());
					}
					tls_held.clear();
''')

patch('engine/priority_dependency.cpp', '''	float closest_distance_sq = 99999.f;''', '''	// TechnoApe ADR 0108: 99999 is 39.5 m in 12.5 cm voxels: farther blocks all got the same priority
	float closest_distance_sq = std::numeric_limits<float>::max();''')
patch('engine/priority_dependency.cpp', '''#include "../util/math/funcs.h"
''', '''#include "../util/math/funcs.h"
#include <limits> // TechnoApe ADR 0108
''')

H = 'util/tasks/threaded_task_runner.h'
patch(H, '''	uint32_t get_thread_count() const {
		return _thread_count;''', '''	uint32_t get_thread_count() const {
		return _active_threads.load(); // TechnoApe ADR 0108''')
patch(H, '''	uint32_t _thread_count = 0;
''', '''	uint32_t _thread_count = 0;
	std::atomic<uint32_t> _active_threads{ 0 }; // TechnoApe ADR 0108: workers at index >= this are parked
''')
patch(C, '''	destroy_all_threads();
	_thread_count = count;
	for (uint32_t i = 0; i < _thread_count; ++i) {
		ThreadData &d = _threads[i];
		create_thread(d, i);
	}
}''', '''	// TechnoApe ADR 0108: fewer threads parks the extra ones, no join
	if (_thread_count > 0 && count <= _thread_count) {
		_active_threads = count;
		return;
	}
	destroy_all_threads();
	_thread_count = count;
	_active_threads = count;
	for (uint32_t i = 0; i < _thread_count; ++i) {
		ThreadData &d = _threads[i];
		create_thread(d, i);
	}
}''')
patch(C, '''	while (!data.stop) {
		bool is_running_serial_task = false;
		bool task_queue_was_empty = false;
''', '''	while (!data.stop) {
		if (data.index >= _active_threads.load(std::memory_order_relaxed)) {
			// TechnoApe ADR 0108: parked (set_thread_count lowered the count); counts as waiting for
			// wait_for_all_tasks
			data.waiting = true;
			Thread::sleep_usec(2000);
			continue;
		}
		data.waiting = false;
		bool is_running_serial_task = false;
		bool task_queue_was_empty = false;
''')
patch(C, '''				_tasks_semaphore.wait();
''', '''				_tasks_semaphore.wait();
				if (data.index >= _active_threads.load(std::memory_order_relaxed)) {
					_tasks_semaphore.post(); // TechnoApe ADR 0108: parked meanwhile: pass the wake-up on
				}
''')
