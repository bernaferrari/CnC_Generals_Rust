// FILE: CommandSetUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, September 2002
// Desc:	 UpgradeModule that sets a new override string for Command Set look ups
///////////////////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "Common/Player.h"
#include "GameClient/ControlBar.h"
#include "GameLogic/Module/CommandSetUpgrade.h"
#include "GameLogic/Object.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void CommandSetUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  UpgradeModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "CommandSet",			INI::parseAsciiString,	NULL, offsetof( CommandSetUpgradeModuleData, m_newCommandSet ) },
		{ "CommandSetAlt",	INI::parseAsciiString,	NULL, offsetof( CommandSetUpgradeModuleData, m_newCommandSetAlt ) },
		{ "TriggerAlt",			INI::parseAsciiString,	NULL, offsetof( CommandSetUpgradeModuleData, m_triggerAlt ) },
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
CommandSetUpgrade::CommandSetUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
CommandSetUpgrade::~CommandSetUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void CommandSetUpgrade::upgradeImplementation( )
{
	Object *obj = getObject();	

	const char * upgradeAlt = getCommandSetUpgradeModuleData()->m_triggerAlt.str();
	const UpgradeTemplate *upgradeTemplate = TheUpgradeCenter->findUpgrade( upgradeAlt );
	
	if (upgradeTemplate)
	{
		UpgradeMaskType upgradeMask = upgradeTemplate->getUpgradeMask();

		// See if upgrade is found in the player completed upgrades
		Player *player = obj->getControllingPlayer();
		if (player)
		{
			UpgradeMaskType playerMask = player->getCompletedUpgradeMask();
			if (playerMask.testForAny(upgradeMask))
			{
				obj->setCommandSetStringOverride( getCommandSetUpgradeModuleData()->m_newCommandSetAlt );
				TheControlBar->markUIDirty();// Refresh the UI in case we are selected
				return;
			}
		}

		// See if upgrade is found in the object completed upgrades
		UpgradeMaskType objMask = obj->getObjectCompletedUpgradeMask();
		if (objMask.testForAny(upgradeMask))
		{
			obj->setCommandSetStringOverride( getCommandSetUpgradeModuleData()->m_newCommandSetAlt );
			TheControlBar->markUIDirty();// Refresh the UI in case we are selected
			return;
		}
	}

	obj->setCommandSetStringOverride( getCommandSetUpgradeModuleData()->m_newCommandSet );
	TheControlBar->markUIDirty();// Refresh the UI in case we are selected
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void CommandSetUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void CommandSetUpgrade::xfer( Xfer *xfer )
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
void CommandSetUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
