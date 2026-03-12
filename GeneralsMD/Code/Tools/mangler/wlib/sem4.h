#ifndef SEM4_HEADER
#define SEM4_HEADER

#include <limits.h>
#ifndef _WINDOWS
#include <unistd.h>
#endif
#include "wstypes.h"

#ifdef _REENTRANT
#ifndef _WINDOWS
#include <semaphore.h>
#else
#include <windows.h>
#endif // _WINDOWS
#endif // _REENTRANT

// Windows headers have a tendency to redefine IN
#ifdef IN
#undef IN
#endif
#define IN const

class Sem4
{
 private:
  #ifdef _REENTRANT
#ifndef _WINDOWS
  sem_t sem;
#else
  HANDLE sem;
#endif
  #endif
 public:
               Sem4();
               Sem4(uint32 value);
              ~Sem4();

  sint32       Wait(void) const;
  sint32       TryWait(void) const;
  sint32       Post(void) const;
  sint32       GetValue(int *sval) const;
  sint32       Destroy(void);
};

#endif
