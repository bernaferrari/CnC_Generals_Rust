// FILE: GrantScienceUpgrade.cpp /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Los Angeles
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2003 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	  August 2, 2003
//
//	Filename: 	GrantScienceUpgrade.cpp
//
//	Author:		  Kris Morness
//	
//	Purpose:	  Grants specified science once requirements met (typically an upgrade).
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Player.h"
#include "Common/Xfer.h"

#include "GameLogic/Object.h"

#include "GameLogic/Module/GrantScienceUpgrade.h"

#ifdef _INTERNAL
// for occasional debugging...
//#pragma optimize("", off)
//#pragma MESSAGE("************************************** WARNING, optimization disabled for debugging purposes")
#endif

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
void GrantScienceUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  UpgradeModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "GrantScience",		INI::parseAsciiString,	NULL, offsetof( GrantScienceUpgradeModuleData, m_grantScienceName ) },
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
GrantScienceUpgrade::GrantScienceUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
	m_scienceType = SCIENCE_INVALID;
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
GrantScienceUpgrade::~GrantScienceUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void GrantScienceUpgrade::upgradeImplementation( )
{
	if( m_scienceType == SCIENCE_INVALID )
	{
		const GrantScienceUpgradeModuleData *data = getGrantScienceUpgradeModuleData();
		m_scienceType = TheScienceStore->getScienceFromInternalName( data->m_grantScienceName );
	}

	Object *obj = getObject();	
	Player *player = obj->getControllingPlayer();
	if( player )
	{
		player->grantScience( m_scienceType );
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void GrantScienceUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void GrantScienceUpgrade::xfer( Xfer *xfer )
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
void GrantScienceUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
