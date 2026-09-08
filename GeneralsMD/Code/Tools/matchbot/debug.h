// FILE: debug.h //////////////////////////////////////////////////////////////
// Minimal debug info
// Author: Matthew D. Campbell, Sept 2002

#ifndef __DEBUG_H__
#define __DEBUG_H__

#ifdef DEBUG

#include <cstdarg>

#ifdef __cplusplus
extern "C" {
#endif
void DebugLog( const char *fmt, ... );
#ifdef __cplusplus
}
#endif
#define DEBUG_LOG(x) DebugLog x

#else // DEBUG

#define DEBUG_LOG(x) {}

#endif // DEBUG

#endif // __DEBUG_H__

