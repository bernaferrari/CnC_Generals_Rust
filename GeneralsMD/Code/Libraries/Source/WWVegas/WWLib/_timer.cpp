#include	"always.h"
#include	"_timer.h"


/***************************************************************************
**	Game frame timer (this is synced between processes).
*/
CDTimerClass<SystemTimerClass> FrameTimer;

/***************************************************************************
** Tick Count global timer object.
*/
TTimerClass<SystemTimerClass> TickCount = 0;


