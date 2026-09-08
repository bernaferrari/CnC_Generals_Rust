/////////////////////////////////////////////////////////////////////////EA-V1
// $File: //depot/GeneralsMD/Staging/code/Libraries/Source/profile/internal.h $
// $Author: mhoffe $
// $Revision: #3 $
// $DateTime: 2003/07/09 10:57:23 $
//
// ©2003 Electronic Arts
//
// Internal header
//////////////////////////////////////////////////////////////////////////////
#ifdef _MSC_VER
#  pragma once
#endif
#ifndef INTERNAL_H // Include guard
#define INTERNAL_H

#include "../debug/debug.h"
#include "internal_funclevel.h"
#include "internal_highlevel.h"
#include "internal_cmd.h"
#include "internal_result.h"

class ProfileFastCS
{
  ProfileFastCS(const ProfileFastCS&);
  ProfileFastCS& operator=(const ProfileFastCS&);

	volatile unsigned m_Flag;
  static HANDLE testEvent;

	void ThreadSafeSetFlag()
	{
		volatile unsigned& nFlag=m_Flag;

		#define ts_lock _emit 0xF0
		DASSERT(((unsigned)&nFlag % 4) == 0);

		__asm mov ebx, [nFlag]
		__asm ts_lock
		__asm bts dword ptr [ebx], 0
		__asm jc The_Bit_Was_Previously_Set_So_Try_Again
		return;

	The_Bit_Was_Previously_Set_So_Try_Again:
    // can't use SwitchToThread() here because Win9X doesn't have it!
    if (testEvent)
		  ::WaitForSingleObject(testEvent,1);
		__asm mov ebx, [nFlag]
		__asm ts_lock
		__asm bts dword ptr [ebx], 0
		__asm jc  The_Bit_Was_Previously_Set_So_Try_Again
	}

	void ThreadSafeClearFlag()
	{
		m_Flag=0;
	}

public:
	ProfileFastCS(void):
    m_Flag(0) 
  {
  }

	class Lock
	{
    Lock(const Lock&);
    Lock& operator=(const Lock&);

		ProfileFastCS& CriticalSection;

	public:
		Lock(ProfileFastCS& cs): 
      CriticalSection(cs)
		{
			CriticalSection.ThreadSafeSetFlag();
		}

		~Lock()
		{
			CriticalSection.ThreadSafeClearFlag();
		}
	};

	friend class Lock;
};

void *ProfileAllocMemory(unsigned numBytes);
void *ProfileReAllocMemory(void *oldPtr, unsigned newSize);
void ProfileFreeMemory(void *ptr);

__forceinline void ProfileGetTime(__int64 &t)
{
  _asm
  {
    mov ecx,[t]
    push eax
    push edx
    rdtsc
    mov [ecx],eax
    mov [ecx+4],edx
    pop edx
    pop eax
  };
}

#endif // INTERNAL_H
