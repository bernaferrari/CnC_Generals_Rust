#include	"always.h"
#include	"stimer.h"
#include	"win.h"

#ifdef _MSC_VER
#pragma warning (push,3)
#endif

#include "systimer.h"

#ifdef _MSC_VER
#pragma warning (pop)
#endif


long SystemTimerClass::operator () (void) const
{
	return TIMEGETTIME()/16;
}


SystemTimerClass::operator long (void) const
{
	return TIMEGETTIME()/16;
}