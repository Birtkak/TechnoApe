# --- a waiting writer goes before new readers (ADR 0136) ---
# Part of the TechnoApe godot_voxel patches. SpatialLock3D had no writer preference: the main thread's paste
# (VoxelToolLodTerrain::paste -> VoxelData::paste, a sim bubble push or an edit) waited in lock_write while mesh tasks,
# column reads and block saves kept taking overlapping read boxes, and in flight a stream of them starved it for
# 169-512 ms frames. Now a writer that cannot lock at once registers its box, and new read locks overlapping a
# registered box wait until the writer is through; reads already held finish as before. A thread never holds two
# boxes of one lock (the lock's own rule), so a reader waiting behind a writer holds nothing the writer waits for.
import sys
root = sys.argv[1]

def patch(rel, old, new):
    p = root + '/' + rel
    s = open(p, encoding='utf-8').read()
    if s.count(new) == 1 and (new.count(old) or s.count(old) == 0):
        return  # already done
    assert s.count(old) == 1, (rel, old)
    s = s.replace(old, new, 1)
    open(p, 'w', encoding='utf-8', newline='\n').write(s)

L = 'util/thread/spatial_lock_3d.h'
patch(L, '''	void lock_write(const BoxBounds3i &box) {
		while (try_lock_write(box) == false) {
			_semaphore.wait();
		}
	}
''', '''	void lock_write(const BoxBounds3i &box) {
		if (try_lock_write(box)) {
			return;
		}
		// TechnoApe ADR 0136: new readers of this box wait behind us (can_lock_for_read)
		_boxes_mutex.lock();
		_waiting_writes.push_back(box);
		_boxes_mutex.unlock();
		while (try_lock_write(box) == false) {
			_semaphore.wait();
		}
		_boxes_mutex.lock();
		for (unsigned int i = 0; i < _waiting_writes.size(); ++i) {
			if (_waiting_writes[i] == box) {
				_waiting_writes[i] = _waiting_writes.back();
				_waiting_writes.pop_back();
				break;
			}
		}
		_boxes_mutex.unlock();
	}
''')
patch(L, '''			if (existing_box.bounds.intersects(box) && existing_box.mode == MODE_WRITE) {
				return false;
				break;
			}
		}
		return true;
	}

	bool can_lock_for_write(''', '''			if (existing_box.bounds.intersects(box) && existing_box.mode == MODE_WRITE) {
				return false;
				break;
			}
		}
		for (const BoxBounds3i &w : _waiting_writes) { // TechnoApe ADR 0136: writer preference
			if (w.intersects(box)) {
				return false;
			}
		}
		return true;
	}

	bool can_lock_for_write(''')
patch(L, '''	StdVector<Box> _boxes;
''', '''	StdVector<Box> _boxes;
	// TechnoApe ADR 0136: boxes of writers waiting to lock (new overlapping readers wait behind them)
	StdVector<BoxBounds3i> _waiting_writes;
''')
