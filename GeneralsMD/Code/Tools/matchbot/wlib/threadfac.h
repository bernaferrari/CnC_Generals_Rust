//
// Platform independent thread creation (Win32 & POSIX)
//

#ifndef THREADFAC_HEADER
#define THREADFAC_HEADER

#ifdef _WIN32
  #include <process.h>
#endif
#include "wstypes.h"
#include <stdlib.h>

#ifdef _WIN32
  #include <wtypes.h>
#else // UNIX
  #include <pthread.h>
#endif

// Windows headers have a tendency to redefine IN
#ifdef IN
#undef IN
#endif
#define IN const

#include "critsec.h"



#ifdef THREADFAC_CODE
  // This is the fake thread entry point for functions
  #ifdef _WIN32
    static unsigned __stdcall threadFuncLauncher(void *temp);
  #else  // UNIX
    static void *threadFuncLauncher(void *temp);
  #endif
 
  // Fake entry point for classes
  #ifdef _WIN32
    static unsigned __stdcall threadClassLauncher(void *temp);
  #else  // UNIX
    static void *threadClassLauncher(void *temp);
  #endif
#endif





// Forward definition of base class for threaded classes
class Runnable;

//
// Call the static method startThread to begin a new thread.
//
class ThreadFactory
{
 public:
  static bit8    startThread(void (*start_func)(void *), void *data);
  static bit8    startThread(Runnable &runable, void *data, bit8 destroy=FALSE);
};



//
// Base class for when you want a thread to execute inside a class
//  instead of a function.
//
class Runnable
{
 public:
                Runnable();
   virtual     ~Runnable();


   // ThreadFactory needs to be able to access the private
   // IsRunning_ field.
   friend class ThreadFactory;

   // So do the threadClassLaunchers
   #ifdef _WIN32
      friend static unsigned __stdcall threadClassLauncher(void *temp);
   #else  // UNIX
     friend void *threadClassLauncher(void *temp);
   #endif

   virtual void run(void *data)=0;       // Thread entry point

           void startThread(void *data,bit8 destroy=FALSE)  // nice way to start a thread
           {
             ThreadFactory::startThread(*this,data,destroy);
           };

           // Is there a thread running in this class?
           static bit8 isRunning(void);

           // Get the count of threads running inside this class
           static int    getThreadCount();


  private:
   static int       ThreadCount_;
   static CritSec   CritSec_;           // to protect ThreadCount_
};

#endif
