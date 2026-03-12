// FILE: EnemyNearUpdate.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, December 2002
// Desc:   Reacts when an enemy is within range
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/PerfTimer.h"
#include "Common/ThingTemplate.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/Module/EnemyNearUpdate.h"
#include "GameLogic/Object.h"
#include "GameLogic/AI.h"
#include "GameLogic/Module/AIUpdate.h"

//-------------------------------------------------------------------------------------------------

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
EnemyNearUpdate::EnemyNearUpdate( Thing *thing, const ModuleData* moduleData ) : UpdateModule( thing, moduleData ),
	m_enemyNear(false),
	m_enemyScanDelay(0)
{
	// bias a random amount so everyone doesn't spike at once
	m_enemyScanDelay += GameLogicRandomValue(0, getEnemyNearUpdateModuleData()->m_enemyScanDelayTime);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
EnemyNearUpdate::~EnemyNearUpdate( void )
{
}


//-------------------------------------------------------------------------------------------------
/** 
 * Look around us for enemies.
 */
void EnemyNearUpdate::checkForEnemies( void )
{
	// periodic enemy checks
	if (m_enemyScanDelay == 0)
	{
		m_enemyScanDelay = getEnemyNearUpdateModuleData()->m_enemyScanDelayTime;

		Real visionRange = getObject()->getVisionRange();
		Object* enemy = TheAI->findClosestEnemy( getObject(), visionRange, AI::CAN_SEE );
		m_enemyNear = (enemy != NULL);
	}
	else
	{
		--m_enemyScanDelay;
	}
}

//-------------------------------------------------------------------------------------------------
///< Sit around until an enemy gets near.
//-------------------------------------------------------------------------------------------------
UpdateSleepTime EnemyNearUpdate::update()
{
/// @todo srj use SLEEPY_UPDATE here
	Bool enemyWasNear = m_enemyNear;

	checkForEnemies();

	if (m_enemyNear && !enemyWasNear)
	{
		// change the state of the art to an "enemy near" state
		Drawable *draw = getObject()->getDrawable();
		if( draw )
			draw->setModelConditionState( MODELCONDITION_ENEMYNEAR );
	}
	else if (!m_enemyNear && enemyWasNear)
	{
		// change the state of the art to an idle state
		Drawable *draw = getObject()->getDrawable();
		if( draw )
			draw->clearModelConditionState( MODELCONDITION_ENEMYNEAR );
	}
	return UPDATE_SLEEP_NONE;
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void EnemyNearUpdate::crc( Xfer *xfer )
{

	// extend base class
	UpdateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void EnemyNearUpdate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpdateModule::xfer( xfer );

	// enemy scan delay
	xfer->xferUnsignedInt( &m_enemyScanDelay );

	// enemy near
	xfer->xferBool( &m_enemyNear );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void EnemyNearUpdate::loadPostProcess( void )
{

	// extend base class
	UpdateModule::loadPostProcess();

}  // end loadPostProcess
