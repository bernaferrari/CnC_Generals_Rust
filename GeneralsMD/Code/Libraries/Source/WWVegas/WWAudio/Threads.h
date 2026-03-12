#if defined(_MSC_VER)
#pragma once
#endif

#ifndef __WWAUDIO_THREADS_H
#define __WWAUDIO_THREADS_H

#include "Windows.H"
#include "Vector.H"
#include "mutex.h"

// Forward declarations
class RefCountClass;


//////////////////////////////////////////////////////////////////////////
//
//	WWAudioThreadsClass
//
//	Simple class that provides a common namespace for tying thread
// information together.
//
//////////////////////////////////////////////////////////////////////////
class WWAudioThreadsClass
{
	public:

		//////////////////////////////////////////////////////////////////////
		//	Public constructors/destructors
		//////////////////////////////////////////////////////////////////////
		WWAudioThreadsClass (void);
		~WWAudioThreadsClass (void);

		//////////////////////////////////////////////////////////////////////
		//	Public methods
		//////////////////////////////////////////////////////////////////////
		
		//
		//	Delayed release mechanism
		//
		static HANDLE		Create_Delayed_Release_Thread (LPVOID param = NULL);
		static void			End_Delayed_Release_Thread (DWORD timeout = 20000);
		static void			Add_Delayed_Release_Object (RefCountClass *object, DWORD delay = 2000);
		static void			Flush_Delayed_Release_Objects (void);

	private:

		//////////////////////////////////////////////////////////////////////
		//	Private methods
		//////////////////////////////////////////////////////////////////////
		static void	__cdecl Delayed_Release_Thread_Proc (LPVOID param);

		//////////////////////////////////////////////////////////////////////
		//	Private data types
		//////////////////////////////////////////////////////////////////////
		typedef struct _DELAYED_RELEASE_INFO
		{
			RefCountClass *	object;
			DWORD					time;

			_DELAYED_RELEASE_INFO *next;

		} DELAYED_RELEASE_INFO;

		//typedef DynamicVectorClass<DELAYED_RELEASE_INFO *>	RELEASE_LIST;

		//////////////////////////////////////////////////////////////////////
		//	Private member data
		//////////////////////////////////////////////////////////////////////
		static HANDLE						m_hDelayedReleaseThread;
		static HANDLE						m_hDelayedReleaseEvent;
		//static RELEASE_LIST		m_ReleaseList;
		static CriticalSectionClass	m_CriticalSection;
		static DELAYED_RELEASE_INFO *	m_ReleaseListHead;
		static CriticalSectionClass	m_ListMutex;
		static bool							m_IsShuttingDown;
};

#endif //__WWAUDIO_THREADS_H

