// FILE: UnpauseSpecialPowerUpgrade.cpp ///////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:	 An upgrade that starts the timer on a Special Power module, so you can have them 
// dependent on upgrades on the logic side, like NEED_UPGRADE does on the client side by disabling
// the button.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"
#include "Common/SpecialPower.h"
#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SpecialPowerModule.h"
#include "GameLogic/Module/UnpauseSpecialPowerUpgrade.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnpauseSpecialPowerUpgradeModuleData::UnpauseSpecialPowerUpgradeModuleData( void )
{
	m_specialPower = NULL;
} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
/* static */ void UnpauseSpecialPowerUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p)
{
	UpgradeModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "SpecialPowerTemplate", INI::parseSpecialPowerTemplate, NULL, offsetof( UnpauseSpecialPowerUpgradeModuleData, m_specialPower ) },
		{ 0, 0, 0, 0 } 
	};
	p.add(dataFieldParse);

}  // end buildFieldParse

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnpauseSpecialPowerUpgrade::UnpauseSpecialPowerUpgrade( Thing *thing, const ModuleData* moduleData ) : 
							UpgradeModule( thing, moduleData )
{

}  

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
UnpauseSpecialPowerUpgrade::~UnpauseSpecialPowerUpgrade( void )
{

} 

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void UnpauseSpecialPowerUpgrade::upgradeImplementation( void )
{
	for (BehaviorModule** m = getObject()->getBehaviorModules(); *m; ++m)
	{
		SpecialPowerModuleInterface* sp = (*m)->getSpecialPower();
		if (!sp)
			continue;

		if( sp->getSpecialPowerTemplate() == getUnpauseSpecialPowerUpgradeModuleData()->m_specialPower )
			sp->pauseCountdown( FALSE );
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void UnpauseSpecialPowerUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void UnpauseSpecialPowerUpgrade::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpgradeModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void UnpauseSpecialPowerUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
