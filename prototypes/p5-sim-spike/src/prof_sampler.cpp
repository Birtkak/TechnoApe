// A sampling profiler for the generator bench (ADR 0135): a thread suspends the thread that started it every
// millisecond and keeps its call stack; game/tools/gen_prof.py maps the addresses to source lines with
// addr2line (build with debug_symbols=yes). Windows only; elsewhere it returns nothing.
#include "fine_gen.h"

#include <atomic>
#include <thread>
#include <vector>

#ifdef _WIN32
#define NOMINMAX
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

using namespace godot;

namespace {
std::atomic<bool> s_on{false};
std::thread s_thr;
std::vector<uint64_t> s_chains;   // per sample: frame count, then the frames' instruction pointers, innermost first
}

void FineGen::prof_start() {
#ifdef _WIN32
    if (s_on) return;
    HANDLE target = nullptr;
    DuplicateHandle(GetCurrentProcess(), GetCurrentThread(), GetCurrentProcess(), &target, THREAD_ALL_ACCESS, FALSE, 0);
    s_chains.clear();
    s_on = true;
    s_thr = std::thread([target]() {
        while (s_on) {
            if (SuspendThread(target) != (DWORD)-1) {
                CONTEXT c;
                c.ContextFlags = CONTEXT_FULL;
                if (GetThreadContext(target, &c)) {
                    // walk the stack with the x64 unwind tables (GCC emits them too)
                    uint64_t f[32];
                    int n = 0;
                    while (n < 32 && c.Rip) {
                        f[n++] = c.Rip;
                        DWORD64 image = 0;
                        PRUNTIME_FUNCTION fe = RtlLookupFunctionEntry(c.Rip, &image, nullptr);
                        if (!fe) { c.Rip = *(DWORD64 *)c.Rsp; c.Rsp += 8; continue; }   // a leaf
                        void *hd = nullptr;
                        DWORD64 ef = 0;
                        RtlVirtualUnwind(UNW_FLAG_NHANDLER, image, c.Rip, fe, &c, &hd, &ef, nullptr);
                    }
                    s_chains.push_back((uint64_t)n);
                    s_chains.insert(s_chains.end(), f, f + n);
                }
                ResumeThread(target);
            }
            Sleep(1);
        }
        CloseHandle(target);
    });
#endif
}

PackedInt64Array FineGen::prof_stop() {
    PackedInt64Array out;
#ifdef _WIN32
    if (!s_on) return out;
    s_on = false;
    s_thr.join();
    HMODULE mod = nullptr;
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCSTR)&FineGen::prof_stop, &mod);
    out.push_back((int64_t)(uintptr_t)mod);   // [module base, n, n frames innermost first, n, ...]
    for (const uint64_t v : s_chains) out.push_back((int64_t)v);
#endif
    return out;
}

int64_t FineGen::thread_cpu_us() {
#ifdef _WIN32
    FILETIME a, b, k, u;
    if (!GetThreadTimes(GetCurrentThread(), &a, &b, &k, &u)) return 0;
    return (int64_t)((((uint64_t)u.dwHighDateTime << 32) | u.dwLowDateTime) / 10);   // user time, 100 ns units
#else
    return 0;
#endif
}
