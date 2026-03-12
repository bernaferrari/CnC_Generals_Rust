/*****************************************************************************
**            Includes                                                      **
*****************************************************************************/

#include <wpaudio/altypes.h>
#include <wpaudio/level.h>
#include <wpaudio/time.h>

// 'assignment within condition expression'.
#pragma warning(disable : 4706)


DBG_DECLARE_TYPE ( AudioLevel )

/*****************************************************************************
**          Externals                                                       **
*****************************************************************************/



/*****************************************************************************
**           Defines                                                        **
*****************************************************************************/



/*****************************************************************************
**        Private Types                                                     **
*****************************************************************************/



/*****************************************************************************
**         Private Data                                                     **
*****************************************************************************/



/*****************************************************************************
**         Public Data                                                      **
*****************************************************************************/



/*****************************************************************************
**         Private Prototypes                                               **
*****************************************************************************/



/*****************************************************************************
**          Private Functions                                               **
*****************************************************************************/



/*****************************************************************************
**          Public Functions                                                **
*****************************************************************************/

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		AudioLevelInit ( AudioLevel *level, int startLevel )
{

	DBG_ASSERT ( level != NULL );
	DBG_SET_TYPE ( level, AudioLevel );

	level->flags = 0;
	level->lastTime = AudioGetTime ();
	AudioLevelSetDuration ( level, SECONDS(1), AUDIO_LEVEL_MAX);
	AudioLevelSet ( level, startLevel );
	AudioLevelUpdate ( level );

}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		AudioLevelSet ( AudioLevel *level, int newLevel )
{

	DBG_ASSERT_TYPE ( level, AudioLevel );
	DBG_ASSERT (newLevel>=AUDIO_LEVEL_MIN);
	DBG_ASSERT (newLevel<=AUDIO_LEVEL_MAX);

	level->flags |= AUDIO_LEVEL_SET;
	level->newLevel = (newLevel<<AUDIO_LEVEL_SCALE);

}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		AudioLevelForce( AudioLevel *level)
{

	DBG_ASSERT_TYPE ( level, AudioLevel );

	level->flags |= AUDIO_LEVEL_SET;

}

#ifdef _DEBUG

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

int			AudioLevelApply ( AudioLevel *level, int val )
{

	DBG_ASSERT_TYPE ( level, AudioLevel );
	DBG_ASSERT (val >= AUDIO_LEVEL_MIN_VAL);
	DBG_ASSERT (val <= AUDIO_LEVEL_MAX_VAL);

	return AUDIO_LEVEL_APPLY(level,val);
}

#endif

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void		AudioLevelAdjust ( AudioLevel *level, int newLevel )
{

	DBG_ASSERT_TYPE ( level, AudioLevel );
	DBG_ASSERT (newLevel>=AUDIO_LEVEL_MIN);
	DBG_ASSERT (newLevel<=AUDIO_LEVEL_MAX);

	level->flags &= ~AUDIO_LEVEL_SET;
	if ( level->newLevel == level->level)
	{
		level->lastTime = AudioGetTime ();
	}
	level->newLevel = newLevel<<AUDIO_LEVEL_SCALE;


}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

void	AudioLevelSetDuration ( AudioLevel *level, TimeStamp time, int range )
{

	DBG_ASSERT_TYPE ( level, AudioLevel );
	DBG_ASSERT ( time != 0 );
	DBG_ASSERT (range > 0);
	DBG_ASSERT (range <= AUDIO_LEVEL_MAX);

	level->change = (range<< AUDIO_LEVEL_SCALE) / (uint) time;
	level->duration = time;

}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

int		AudioLevelUpdate ( AudioLevel *level )
{
	int	dif;
	int	delta;	//  amount to move by this update 
	TimeStamp	time, thisTime;


	DBG_ASSERT_TYPE ( level, AudioLevel );

	if ( (dif = (level->newLevel - level->level)) )
	{
	 	if (level->flags & AUDIO_LEVEL_SET )
		{
			level->level = level->newLevel;
		}
		else
		{

			//  calculate what the delta change is for this update 
			thisTime = AudioGetTime ( ) ;
			time = thisTime - level->lastTime;
			level->lastTime = thisTime;	//  remember time of this update 
			
			//  the next check avoid overflowing the delta 
			if (time > level->duration)
			{
				time = level->duration;
			}
			
			delta = level->change * (uint) time;

			if (dif<0)
			{
				if ( delta > (-dif))
				{
					level->level += dif;
				}
				else
				{
					level->level -= delta;
				}
			}
			else
			{
				if ( delta  > dif )
				{
					level->level += dif;
				}
				else
				{
					level->level += delta;
				}
			}
		}
		//  there was a change in the level 
		level->flags |= AUDIO_LEVEL_CHANGED;
		return TRUE;
	}

	//  there has been no change this update 
	return FALSE;
}

/******************************************************************/
/*                                                                */
/*                                                                */
/******************************************************************/

int	AudioLevelGetVal( AudioLevel *level )
{
	return (level->newLevel>>AUDIO_LEVEL_SCALE);
}

