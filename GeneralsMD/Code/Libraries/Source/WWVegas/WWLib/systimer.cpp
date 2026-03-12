#include "systimer.h"

SysTimeClass SystemTime;


/***********************************************************************************************
 * SysTimeClass::SysTimeClass -- default constructor, sets resolution                          *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:    Nothing                                                                           *
 *                                                                                             *
 * OUTPUT:   Nothing                                                                           *
 *                                                                                             *
 * WARNINGS: None                                                                              *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   01/04/2003 : Created by Mark Wilczynski (EAP)                                             *
 *=============================================================================================*/
SysTimeClass::SysTimeClass(void)
{
	//tell windows we need single ms precision.
	timeBeginPeriod(1);
}

/***********************************************************************************************
 * SysTimeClass::~SysTimeClass -- default destructor, resets resolution                         *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:    Nothing                                                                           *
 *                                                                                             *
 * OUTPUT:   Nothing                                                                           *
 *                                                                                             *
 * WARNINGS: None                                                                              *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   01/04/2003 : Created by Mark Wilczynski (EAP)                                             *
 *=============================================================================================*/
SysTimeClass::~SysTimeClass(void)
{
	//tell windows we need single ms precision.
	timeEndPeriod(1);
}

/***********************************************************************************************
 * SysTimeClass::Reset -- Reset class to good state                                            *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:    Nothing                                                                           *
 *                                                                                             *
 * OUTPUT:   Nothing                                                                           *
 *                                                                                             *
 * WARNINGS: None                                                                              *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   12/9/2001 5:51PM ST : Created                                                             *
 *=============================================================================================*/
void SysTimeClass::Reset(void)
{
	StartTime = timeGetTime();
	WrapAdd = 0 - StartTime;
}



/***********************************************************************************************
 * SysTimeClass::Is_Getting_Late -- Are we running out of timer time?                          *
 *                                                                                             *
 *                                                                                             *
 *                                                                                             *
 * INPUT:    Nothing                                                                           *
 *                                                                                             *
 * OUTPUT:   Nothing                                                                           *
 *                                                                                             *
 * WARNINGS: None                                                                              *
 *                                                                                             *
 * HISTORY:                                                                                    *
 *   12/9/2001 6:04PM ST : Created                                                             *
 *=============================================================================================*/
bool SysTimeClass::Is_Getting_Late(void)
{
	/*
	** Even though the timers are all unsigned so we have a max time of 0xffffffff the game casts it to int in various places
	** so it's safer to assume a signed max value.
	*/
	if (Get() > 0x6fffffff) {
		return(true);
	}
	return(false);
}

























