// FILE: AnimationSteeringUpdate.cpp //////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, May 2003
// Desc:   Uses animation states to handle steering.
///////////////////////////////////////////////////////////////////////////////////////////////////

// USER INCLUDES //////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ModelState.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/GameLogic.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/AnimationSteeringUpdate.h"
#include "GameLogic/Module/PhysicsUpdate.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
AnimationSteeringUpdateModuleData::AnimationSteeringUpdateModuleData( void )
{
	m_transitionFrames = 0;
}

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
AnimationSteeringUpdate::AnimationSteeringUpdate( Thing *thing, const ModuleData *moduleData )
												: UpdateModule( thing, moduleData )
{
	m_currentTurnAnim = MODELCONDITION_INVALID;
	m_nextTransitionFrame = 0;
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
AnimationSteeringUpdate::~AnimationSteeringUpdate( void )
{

}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UpdateSleepTime AnimationSteeringUpdate::update( void )
{

	const AnimationSteeringUpdateModuleData *data = getAnimationSteeringUpdateModuleData();
	PhysicsBehavior *physics = getObject()->getPhysics();
	Drawable *draw = getObject()->getDrawable();
	UnsignedInt now = TheGameLogic->getFrame();

	if( physics && draw && now >= m_nextTransitionFrame )
	{
		PhysicsTurningType currentTurn = physics->getTurning();

		switch( m_currentTurnAnim )
		{
			case MODELCONDITION_INVALID:
				//We're currently going straight. Check if we want to turn.
				if( currentTurn == TURN_NEGATIVE )
				{
					//Initiate a right turn
					draw->setModelConditionState( MODELCONDITION_CENTER_TO_RIGHT );
					m_nextTransitionFrame = now + data->m_transitionFrames;
					m_currentTurnAnim = MODELCONDITION_CENTER_TO_RIGHT;
				}
				else if( currentTurn == TURN_POSITIVE )
				{
					//Initiate a left turn
					draw->setModelConditionState( MODELCONDITION_CENTER_TO_LEFT );
					m_nextTransitionFrame = now + data->m_transitionFrames;
					m_currentTurnAnim = MODELCONDITION_CENTER_TO_LEFT;
				}
				break;
			case MODELCONDITION_CENTER_TO_RIGHT:
				//We're currently initiating a turn to the right. The only thing
				//we can do go back to center or maintain the turn.
				if( currentTurn != TURN_NEGATIVE )
				{
					//Recenter!
					draw->clearAndSetModelConditionState( MODELCONDITION_CENTER_TO_RIGHT, MODELCONDITION_RIGHT_TO_CENTER );
					m_nextTransitionFrame = now + data->m_transitionFrames;
					m_currentTurnAnim = MODELCONDITION_RIGHT_TO_CENTER;
				}
				break;
			case MODELCONDITION_CENTER_TO_LEFT:
				//We're currently initiating a turn to the left. The only thing
				//we can do go back to center or maintain the turn.
				if( currentTurn != TURN_POSITIVE )
				{
					//Recenter!
					draw->clearAndSetModelConditionState( MODELCONDITION_CENTER_TO_LEFT, MODELCONDITION_LEFT_TO_CENTER );
					m_nextTransitionFrame = now + data->m_transitionFrames;
					m_currentTurnAnim = MODELCONDITION_LEFT_TO_CENTER;
				}
				break;
			case MODELCONDITION_LEFT_TO_CENTER:
			case MODELCONDITION_RIGHT_TO_CENTER:
				if( currentTurn == TURN_NONE )
				{
					//Finish the turn
					draw->clearModelConditionFlags( MAKE_MODELCONDITION_MASK2( MODELCONDITION_LEFT_TO_CENTER, MODELCONDITION_RIGHT_TO_CENTER ) );
					m_nextTransitionFrame = now;
					m_currentTurnAnim = MODELCONDITION_INVALID;
				}
				break;
		}
	}

	return UPDATE_SLEEP_NONE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void AnimationSteeringUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void AnimationSteeringUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );
}

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void AnimationSteeringUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}

