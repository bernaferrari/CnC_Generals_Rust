// FILE: GrantUpgradeCreate.cpp ////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, April 2002
// Desc:   GrantUpgrade create module
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_OBJECT_STATUS_NAMES
#include "Common/Player.h"
#include "Common/Upgrade.h"
#include "Common/Xfer.h"
#include "GameLogic/Module/GrantUpgradeCreate.h"
#include "GameLogic/Object.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
GrantUpgradeCreateModuleData::GrantUpgradeCreateModuleData()
{
	m_upgradeName = "";
}

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GrantUpgradeCreateModuleData::buildFieldParse(MultiIniFieldParse& p)
{
  CreateModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "UpgradeToGrant",	INI::parseAsciiString,							NULL, offsetof( GrantUpgradeCreateModuleData, m_upgradeName ) },
		{ "ExemptStatus",		ObjectStatusMaskType::parseFromINI, NULL, offsetof( GrantUpgradeCreateModuleData, m_exemptStatus ) },
		{ 0, 0, 0, 0 }
	};

  p.add(dataFieldParse);
}

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
GrantUpgradeCreate::GrantUpgradeCreate( Thing *thing, const ModuleData* moduleData ) : CreateModule( thing, moduleData )
{
}  // end GrantUpgradeCreate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
GrantUpgradeCreate::~GrantUpgradeCreate( void )
{

}  // end ~GrantUpgradeCreate

//-------------------------------------------------------------------------------------------------
/** The create callback. */
//-------------------------------------------------------------------------------------------------
void GrantUpgradeCreate::onCreate( void )
{

	ObjectStatusMaskType exemptStatus = getGrantUpgradeCreateModuleData()->m_exemptStatus;
	ObjectStatusMaskType currentStatus = getObject()->getStatusBits();
	if( exemptStatus.test( OBJECT_STATUS_UNDER_CONSTRUCTION ) )
	{
		if(	!currentStatus.test( OBJECT_STATUS_UNDER_CONSTRUCTION ) ) 
		{
			const UpgradeTemplate *upgradeTemplate = TheUpgradeCenter->findUpgrade( getGrantUpgradeCreateModuleData()->m_upgradeName );
			if( !upgradeTemplate )
			{
				DEBUG_ASSERTCRASH( 0, ("GrantUpdateCreate for %s can't find upgrade template %s.", getObject()->getName(), getGrantUpgradeCreateModuleData()->m_upgradeName ) );
				return;
			}

			Player *player = getObject()->getControllingPlayer();
			if( upgradeTemplate->getUpgradeType() == UPGRADE_TYPE_PLAYER )
			{
				// get the player
				player->addUpgrade( upgradeTemplate, UPGRADE_STATUS_COMPLETE );
			}
			else
			{
				getObject()->giveUpgrade( upgradeTemplate );
			}
			
			player->getAcademyStats()->recordUpgrade( upgradeTemplate, TRUE );
		}
	}

}  // end onCreate

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void GrantUpgradeCreate::onBuildComplete( void )
{
	if( ! shouldDoOnBuildComplete() )
		return;

	CreateModule::onBuildComplete(); // extend

	const UpgradeTemplate *upgradeTemplate = TheUpgradeCenter->findUpgrade( getGrantUpgradeCreateModuleData()->m_upgradeName );
	if( !upgradeTemplate )
	{
		DEBUG_ASSERTCRASH( 0, ("GrantUpdateCreate for %s can't find upgrade template %s.", getObject()->getName(), getGrantUpgradeCreateModuleData()->m_upgradeName ) );
		return;
	}

	if( upgradeTemplate->getUpgradeType() == UPGRADE_TYPE_PLAYER )
	{
		// get the player
		Player *player = getObject()->getControllingPlayer();
		player->addUpgrade( upgradeTemplate, UPGRADE_STATUS_COMPLETE );
	}
	else
	{
		getObject()->giveUpgrade( upgradeTemplate );
	}
}  // end onBuildComplete

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void GrantUpgradeCreate::crc( Xfer *xfer )
{

	// extend base class
	CreateModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void GrantUpgradeCreate::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	CreateModule::xfer( xfer );

}  // end xfer

// ------------------------------------------------------------------------------------------------
/** Load post process */
// ------------------------------------------------------------------------------------------------
void GrantUpgradeCreate::loadPostProcess( void )
{

	// extend base class
	CreateModule::loadPostProcess();

}  // end loadPostProcess
