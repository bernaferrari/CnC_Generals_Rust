#pragma once

// The original implementation only needs these diagnostics for optional
// debug logging. Keep the behavior of the algorithm while making diagnostics
// inert in this standalone producer.
#define DEBUG_LOG(args) ((void)0)
#define DEBUG_ASSERTCRASH(condition, args) ((void)0)
#define DEBUG_ASSERTLOG(condition, args) ((void)0)
#define DEBUG_CRASH(args) ((void)0)
