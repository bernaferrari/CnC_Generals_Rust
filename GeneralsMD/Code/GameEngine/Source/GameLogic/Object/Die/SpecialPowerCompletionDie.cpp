// FILE: SpecialPowerCompletionDie.cpp ////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, May 2002
// Desc:   Die method responsible for telling TheScriptEngine that a special power has been completed
///////////////////////////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/SpecialPower.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/SpecialPowerCompletionDie.h"
#include "GameLogic/Object.h"
#include "GameLogic/ScriptEngine.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SpecialPowerCompletionDie::SpecialPowerCompletionDie( Thing *thing, const ModuleData* moduleData ) : DieModule( thing, moduleData )
{
	m_creatorID = INVALID_ID;
	m_creatorSet = FALSE;
} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SpecialPowerCompletionDie::~SpecialPowerCompletionDie( void )
{

} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::onDie( const DamageInfo *damageInfo )
{
	if (!isDieApplicable(damageInfo))
		return;
	notifyScriptEngine();
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::notifyScriptEngine( void )
{
	if (m_creatorID != INVALID_ID)
	{
		TheScriptEngine->notifyOfCompletedSpecialPower(
			getObject()->getControllingPlayer()->getPlayerIndex(),
			getSpecialPowerCompletionDieModuleData()->m_specialPowerTemplate->getName(),
			m_creatorID);
	}
}  

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::setCreator( ObjectID creatorID )
{
	if (!m_creatorSet)
	{
		m_creatorSet = TRUE;
		m_creatorID = creatorID;
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::crc( Xfer *xfer )
{

	// extend base class
	DieModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	DieModule::xfer( xfer );

	// creator id
	xfer->xferObjectID( &m_creatorID );

	// creator set
	xfer->xferBool( &m_creatorSet );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void SpecialPowerCompletionDie::loadPostProcess( void )
{

	// extend base class
	DieModule::loadPostProcess();

}  // end loadPostProcess
