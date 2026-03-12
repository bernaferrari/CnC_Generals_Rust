#include "mutex.h"
#include "wwdebug.h"
#include <windows.h>


// ----------------------------------------------------------------------------

MutexClass::MutexClass(const char* name) : handle(NULL), locked(false)
{
	#ifdef _UNIX
		//assert(0);
	#else
		handle=CreateMutex(NULL,false,name);
		WWASSERT(handle);
	#endif
}

MutexClass::~MutexClass()
{
	#ifdef _UNIX
		//assert(0);
	#else
		WWASSERT(!locked); // Can't delete locked mutex!
		CloseHandle(handle);
	#endif
}

bool MutexClass::Lock(int time)
{
	#ifdef _UNIX
		//assert(0);
		return true;
	#else
		int res = WaitForSingleObject(handle,time==WAIT_INFINITE ? INFINITE : time);
		if (res!=WAIT_OBJECT_0) return false;
		locked++;
		return true;
	#endif
}

void MutexClass::Unlock()
{
	#ifdef _UNIX
		//assert(0);
	#else
		WWASSERT(locked);
		locked--;
		int res=ReleaseMutex(handle);
		res;	// silence compiler warnings
		WWASSERT(res);
	#endif
}

// ----------------------------------------------------------------------------

MutexClass::LockClass::LockClass(MutexClass& mutex_,int time) : mutex(mutex_)
{
	failed=!mutex.Lock(time);
}

MutexClass::LockClass::~LockClass()
{
	if (!failed) mutex.Unlock();
}







// ----------------------------------------------------------------------------

CriticalSectionClass::CriticalSectionClass() : handle(NULL), locked(false)
{
	#ifdef _UNIX
		//assert(0);
	#else
		handle=W3DNEWARRAY char[sizeof(CRITICAL_SECTION)];
		InitializeCriticalSection((CRITICAL_SECTION*)handle);
	#endif
}

CriticalSectionClass::~CriticalSectionClass()
{
	#ifdef _UNIX
		//assert(0);
	#else
		WWASSERT(!locked); // Can't delete locked mutex!
		DeleteCriticalSection((CRITICAL_SECTION*)handle);
		delete[] handle;
	#endif
}

void CriticalSectionClass::Lock()
{
	#ifdef _UNIX
		//assert(0);
	#else
		EnterCriticalSection((CRITICAL_SECTION*)handle);
		locked++;
	#endif
}

void CriticalSectionClass::Unlock()
{
	#ifdef _UNIX
		//assert(0);
	#else
		WWASSERT(locked);
		locked--;
		LeaveCriticalSection((CRITICAL_SECTION*)handle);
	#endif
}

// ----------------------------------------------------------------------------

CriticalSectionClass::LockClass::LockClass(CriticalSectionClass& critical_section) : CriticalSection(critical_section)
{
	CriticalSection.Lock();
}

CriticalSectionClass::LockClass::~LockClass()
{
	CriticalSection.Unlock();
}


