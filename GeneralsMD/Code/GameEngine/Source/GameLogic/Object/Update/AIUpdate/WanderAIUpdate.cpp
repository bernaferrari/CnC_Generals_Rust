// WanderAIUpdate.cpp //////////
// Will give self random move commands
// Author: Graham Smallwood, April 2002

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/RandomValue.h"
#include "GameLogic/Module/WanderAIUpdate.h"
#include "GameLogic/Object.h"

//-------------------------------------------------------------------------------------------------
AIStateMachine* WanderAIUpdate::makeStateMachine()
{
	return newInstance(AIStateMachine)( getObject(), "WanderAIUpdateMachine");
}

//-------------------------------------------------------------------------------------------------
WanderAIUpdate::WanderAIUpdate( Thing *thing, const ModuleData* moduleData ) : AIUpdateInterface( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
WanderAIUpdate::~WanderAIUpdate( void )
{

}

//-------------------------------------------------------------------------------------------------
UpdateSleepTime WanderAIUpdate::update( void )
{
	// If I'm standing still, move somewhere
	if (isIdle())
	{
		Coord3D dest = *(getObject()->getPosition());
		dest.x += GameLogicRandomValue( 5, 50 );
		dest.y += GameLogicRandomValue( 5, 50 );
 		aiMoveToPosition( &dest, CMD_FROM_AI );
	}

	// extend
	/*UpdateSleepTime ret =*/ AIUpdateInterface::update();
	//return (mine < ret) ? mine : ret;
	/// @todo srj -- someday, make sleepy. for now, must not sleep.
	return UPDATE_SLEEP_NONE;
}  // end update

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void WanderAIUpdate::crc( Xfer *xfer )
{
	// extend base class
	AIUpdateInterface::crc(xfer);
}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void WanderAIUpdate::xfer( Xfer *xfer )
{
  XferVersion currentVersion = 1;
  XferVersion version = currentVersion;
  xfer->xferVersion( &version, currentVersion );
 
 // extend base class
	AIUpdateInterface::xfer(xfer);

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void WanderAIUpdate::loadPostProcess( void )
{
 // extend base class
	AIUpdateInterface::loadPostProcess();
}  // end loadPostProcess
