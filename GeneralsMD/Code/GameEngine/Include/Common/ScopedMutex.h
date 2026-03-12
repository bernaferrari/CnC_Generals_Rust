// FILE: ScopedMutex.h ////////////////////////////////////////////////////////////////////////////
// Author: John McDonald, November 2002
// Desc:   A scoped mutex class to easily lock a scope with a pre-existing mutex object.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SCOPEDMUTEX_H__
#define __SCOPEDMUTEX_H__

class ScopedMutex
{
	private:
		HANDLE m_mutex;

	public:
		ScopedMutex(HANDLE mutex) : m_mutex(mutex)
		{
			DWORD status = WaitForSingleObject(m_mutex, 500);
			if (status != WAIT_OBJECT_0) {
				DEBUG_LOG(("ScopedMutex WaitForSingleObject timed out - status %d\n", status));
			}
		}

		~ScopedMutex()
		{
			ReleaseMutex(m_mutex);
		}
};

#endif /* __SCOPEDMUTEX_H__ */