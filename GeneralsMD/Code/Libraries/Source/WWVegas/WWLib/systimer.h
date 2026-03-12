#pragma once
#ifndef _SYSTIMER_H

#include "always.h"
#include <windows.h>
#include "mmsys.h"

#define TIMEGETTIME SystemTime.Get
#define MS_TIMER_SECOND 1000

/*
** Class that just wraps around timeGetTime()
**
**
*/
class SysTimeClass
{

	public:

		SysTimeClass(void);	//default constructor
		~SysTimeClass();	//default destructor

		/*
		** Get. Use everywhere you would use timeGetTime
		*/
		WWINLINE unsigned long Get(void);
		WWINLINE unsigned long operator () (void) {return(Get());}
		WWINLINE operator unsigned long(void) {return(Get());}

		/*
		** Use periodically (like every few days!) to make sure the timer doesn't wrap.
		*/
		void Reset(void);

		/*
		** See if the timer is about to wrap.
		*/
		bool Is_Getting_Late(void);

	private:

		/*
		** Time we were first called.
		*/
		unsigned long StartTime;

		/*
		** Time to add after timer wraps.
		*/
		unsigned long WrapAdd;

};

extern SysTimeClass SystemTime;


/***********************************************************************************************
 * SysTimeClass::Get -- Wrapper around system timeGetTime() api call                           *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:    Nothing                                                                           *
 *                                                                                             *
 * OUTPUT:   Current system time in ms                                                         *
 *                                                                                             *
 * WARNINGS: None                                                                              *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   10/25/2001 1:38PM ST : Created                                                            *
 *=============================================================================================*/
WWINLINE unsigned long SysTimeClass::Get(void)
{
	/*
	** This has to be static here since we don't know if we will get called in a global constructor of another object before our
	** constructor gets called. In fact, we don't even have a constructor because it's pointless.
	*/
	static bool is_init = false;

	if (!is_init) {
		Reset();
		is_init = true;
	}

	unsigned long time = timeGetTime();
	if (time > StartTime) {
		return(time - StartTime);
	}

	/*
	** Timer wrapped around. Eeek.
	*/
	return(time + WrapAdd);
}



#ifdef timeGetTime
#undef timeGetTime
#define timeGetTime SystemTime.Get
#endif //timeGetTime




#endif //_SYSTIMER_H