#ifndef CRITSEC_HEADER
#define CRITSEC_HEADER

#include "wstypes.h"
#ifdef _WIN32
 #include <windows.h>
 #include <winbase.h>
#elif defined(_UNIX)
  #include <pthread.h>
  #include <errno.h>
#endif

// Windows headers have a tendency to redefine IN
#ifdef IN
#undef IN
#endif
#define IN const

//
// Critical Section built either on a POSIX Mutex, or a Win32 Critical Section
//
// POSIX version is done by keeping a thread_id and a reference count.  Win32 version
//  just calls the built in functions.
//
class CritSec
{
 public:
                CritSec();
               ~CritSec();
    
  sint32		lock(int *refcount=NULL) RO;
  sint32		unlock(void) RO;

 protected:
  #ifdef _WIN32
    mutable CRITICAL_SECTION    CritSec_;
  #else
    mutable pthread_mutex_t	Mutex_;         // Mutex lock
    mutable pthread_t		ThreadId_;	// Owner of mutex
    mutable int                 RefCount_;      // Reference count
  #endif
};

#endif
