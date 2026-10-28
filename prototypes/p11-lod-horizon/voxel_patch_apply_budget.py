# --- finished tasks applied under a time budget per frame (ADR 0105) ---
# Part of the TechnoApe godot_voxel patches: run before voxel_patch_profile.py.
# VoxelEngine::process applied every finished task in one frame. Once generation got faster (a 100 m box
# in flight), a frame could receive ~17,700 finished blocks and spend 68 ms applying them on the main thread.
# The general pool now applies its finished tasks for at most APPLY_BUDGET_USEC per frame and keeps the rest
# in order for the next frame. Shutdown still drains them all.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

H = 'util/tasks/threaded_task_runner.h'
patch(H, '''	template <typename F>
	void dequeue_completed_tasks(F f) {
		ZN_PROFILE_SCOPE();
		StdVector<IThreadedTask *> &temp = get_completed_tasks_temp_tls();
		ZN_ASSERT(temp.size() == 0);
		{
			MutexLock lock(_completed_tasks_mutex);
			append_array(temp, _completed_tasks);
			_completed_tasks.clear();
			// std::move doesn't guarantee preservation of vector capacity
			// temp = std::move(_completed_tasks);
		}
		for (IThreadedTask *task : temp) {
#ifdef ZN_THREADED_TASK_RUNNER_CHECK_DUPLICATE_TASKS
			debug_remove_owned_task(task);
#endif
			f(task);
		}
		temp.clear();
	}''', '''	// TechnoApe: at most this long applying finished tasks per dequeue (0 = all of them); the rest wait in order
	void set_dequeue_budget_usec(uint32_t usec) {
		_dequeue_budget_usec = usec;
	}

	template <typename F>
	void dequeue_completed_tasks(F f) {
		ZN_PROFILE_SCOPE();
		StdVector<IThreadedTask *> &temp = get_completed_tasks_temp_tls();
		ZN_ASSERT(temp.size() == 0);
		{
			MutexLock lock(_completed_tasks_mutex);
			append_array(temp, _completed_tasks);
			_completed_tasks.clear();
			// std::move doesn't guarantee preservation of vector capacity
			// temp = std::move(_completed_tasks);
		}
		const auto t0 = std::chrono::steady_clock::now();
		size_t i = 0;
		for (; i < temp.size(); ++i) {
			if (_dequeue_budget_usec > 0 && (i & 15) == 15 &&
				std::chrono::steady_clock::now() - t0 > std::chrono::microseconds(_dequeue_budget_usec)) {
				break;
			}
			IThreadedTask *task = temp[i];
#ifdef ZN_THREADED_TASK_RUNNER_CHECK_DUPLICATE_TASKS
			debug_remove_owned_task(task);
#endif
			f(task);
		}
		if (i < temp.size()) {
			// over budget: the rest go back in front of what finished meanwhile
			MutexLock lock(_completed_tasks_mutex);
			_completed_tasks.insert(_completed_tasks.begin(), temp.begin() + i, temp.end());
		}
		temp.clear();
	}''')
s = open(root + '/' + H, encoding='utf-8').read()
if '#include <chrono>' not in s:
    i = s.index('#include')
    s = s[:i] + '#include <chrono>\n' + s[i:]
    open(root + '/' + H, 'w', encoding='utf-8', newline='\n').write(s)
s = open(root + '/' + H, encoding='utf-8').read()
old = '\tStdVector<IThreadedTask *> _completed_tasks;'
assert s.count(old) == 1, 'completed tasks member'
s = s.replace(old, old + '\n\tuint32_t _dequeue_budget_usec = 0; // TechnoApe: see set_dequeue_budget_usec', 1)
open(root + '/' + H, 'w', encoding='utf-8', newline='\n').write(s)

E = 'engine/voxel_engine.cpp'
patch(E, '''	_general_thread_pool.set_priority_update_period(200);
''', '''	_general_thread_pool.set_priority_update_period(200);
	_general_thread_pool.set_dequeue_budget_usec(4000); // TechnoApe: ADR 0105
''')
patch(E, '''	_general_thread_pool.wait_for_all_tasks();

	_general_thread_pool.dequeue_completed_tasks([warn]''', '''	_general_thread_pool.wait_for_all_tasks();

	_general_thread_pool.set_dequeue_budget_usec(0); // TechnoApe: shutdown drains everything
	_general_thread_pool.dequeue_completed_tasks([warn]''')

# While the main thread applies finished tasks it raises g_main_applying_results, so a worker about to take a lock
# the main thread also needs (voxel_patch_unload.py: loading_blocks_mutex, in chunks) yields first: Windows
# mutexes are not fair, and a worker re-locking between chunks starved the main thread for 7-16 ms.
s = open(root + '/' + H, encoding='utf-8').read()
old = 'namespace zylann {\n'
assert s.count(old) >= 1
s = s.replace(old, old + '\n// TechnoApe ADR 0105: true while the main thread applies finished tasks (see dequeue_completed_tasks)\ninline std::atomic<bool> g_main_applying_results{ false };\n', 1)
if '#include <atomic>' not in s:
    i = s.index('#include')
    s = s[:i] + '#include <atomic>\n' + s[i:]
old = '''		const auto t0 = std::chrono::steady_clock::now();
		size_t i = 0;
		for (; i < temp.size(); ++i) {'''
assert s.count(old) == 1
s = s.replace(old, '''		const auto t0 = std::chrono::steady_clock::now();
		const bool main_apply = _dequeue_budget_usec > 0;
		if (main_apply) {
			g_main_applying_results.store(true, std::memory_order_relaxed);
		}
		size_t i = 0;
		for (; i < temp.size(); ++i) {''', 1)
old = '''		if (i < temp.size()) {
			// over budget: the rest go back in front of what finished meanwhile'''
assert s.count(old) == 1
s = s.replace(old, '''		if (main_apply) {
			g_main_applying_results.store(false, std::memory_order_relaxed);
		}
		if (i < temp.size()) {
			// over budget: the rest go back in front of what finished meanwhile''', 1)
open(root + '/' + H, 'w', encoding='utf-8', newline='\n').write(s)
