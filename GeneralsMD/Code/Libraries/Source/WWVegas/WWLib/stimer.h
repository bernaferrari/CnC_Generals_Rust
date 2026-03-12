#ifndef STIMER_H
#define STIMER_H

/****************************************************************************
**	Timer constants. These are used when setting the countdown timer.
**	Note that this is based upon a timer that ticks every 60th of a second.
*/
#define	TIMER_SECOND			60
#define	TIMER_MINUTE			(TIMER_SECOND*60)

#define	FADE_PALETTE_FAST		(TIMER_SECOND/8)
#define	FADE_PALETTE_MEDIUM	(TIMER_SECOND/4)
#define	FADE_PALETTE_SLOW		(TIMER_SECOND/2)

#define	TICKS_PER_SECOND		15
#define	TICKS_PER_MINUTE		(TICKS_PER_SECOND * 60)
#define	TICKS_PER_HOUR			(TICKS_PER_MINUTE * 60)

#define	GRAYFADETIME			(1 * TICKS_PER_SECOND)


class SystemTimerClass
{
	public:
		long operator () (void) const;
		operator long (void) const;
};

#endif
