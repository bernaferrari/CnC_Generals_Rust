// CriticalSection.h ///////////////////////////////////////////////////////
// Utility class to use critical sections in areas of code.
// Author: JohnM And MattC, August 13, 2002

#pragma once

#ifndef __CRITICALSECTION_H__
#define __CRITICALSECTION_H__

#include "Common/PerfTimer.h"

#ifdef PERF_TIMERS
extern PerfGather TheCritSecPerfGather;
#endif

class CriticalSection
{
	CRITICAL_SECTION m_windowsCriticalSection;

	public:
		CriticalSection()
		{
			#ifdef PERF_TIMERS
			AutoPerfGather a(TheCritSecPerfGather);
			#endif
			InitializeCriticalSection( &m_windowsCriticalSection );
		}

		virtual ~CriticalSection()
		{
			#ifdef PERF_TIMERS
			AutoPerfGather a(TheCritSecPerfGather);
			#endif
			DeleteCriticalSection( &m_windowsCriticalSection );
		}

	public:	// Use these when entering/exiting a critical section.
		void enter( void ) 
		{ 
			#ifdef PERF_TIMERS
			AutoPerfGather a(TheCritSecPerfGather);
			#endif
			EnterCriticalSection( &m_windowsCriticalSection );
		}
		
		void exit( void )
		{
			#ifdef PERF_TIMERS
			AutoPerfGather a(TheCritSecPerfGather);
			#endif
			LeaveCriticalSection( &m_windowsCriticalSection );
		}
};

class ScopedCriticalSection
{
	private:
		CriticalSection *m_cs;
	
	public:
		ScopedCriticalSection( CriticalSection *cs ) : m_cs(cs)
		{ 
			if (m_cs) 
				m_cs->enter();
		}

		virtual ~ScopedCriticalSection( )
		{ 
			if (m_cs) 
				m_cs->exit();
		}
};

#include "mutex.h"

// These should be NULL on creation then non-NULL in WinMain or equivalent.
// This allows us to be silently non-threadsafe for WB and other single-threaded apps.
extern FastCriticalSectionClass TheAsciiStringCriticalSection;
extern CriticalSection *TheUnicodeStringCriticalSection;
extern CriticalSection *TheDmaCriticalSection;
extern CriticalSection *TheMemoryPoolCriticalSection;
extern CriticalSection *TheDebugLogCriticalSection;

#endif /* __CRITICALSECTION_H__ */
