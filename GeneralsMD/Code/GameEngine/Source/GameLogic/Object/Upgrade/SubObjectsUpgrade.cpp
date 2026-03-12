// FILE: SubObjectsUpgrade.cpp /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	September 2002
//
//	Filename: SubObjectsUpgrade.cpp
//
//	Author:		Kris Morness
//	
//	Purpose:	Shows or hides a list of subobjects based on upgrade statii. It
//            will override any animation subobjects states.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#define DEFINE_OBJECT_STATUS_NAMES
#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/Object.h"
#include "GameLogic/Module/SubObjectsUpgrade.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SubObjectsUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{
  UpgradeModuleData::buildFieldParse(p);

	static const FieldParse dataFieldParse[] = 
	{
		{ "ShowSubObjects", INI::parseAsciiStringVectorAppend, NULL, offsetof( SubObjectsUpgradeModuleData, m_showSubObjectNames ) },
		{ "HideSubObjects", INI::parseAsciiStringVectorAppend, NULL, offsetof( SubObjectsUpgradeModuleData, m_hideSubObjectNames ) },
		{ 0, 0, 0, 0 }
	};
  p.add(dataFieldParse);
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SubObjectsUpgrade::SubObjectsUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
SubObjectsUpgrade::~SubObjectsUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void SubObjectsUpgrade::upgradeImplementation( )
{
	const SubObjectsUpgradeModuleData *data = getSubObjectsUpgradeModuleData();
	UpgradeMaskType activation, conflicting;
	getUpgradeActivationMasks( activation, conflicting );
	
	//First make sure we have the right combination of upgrades

	if( getObject()->getObjectCompletedUpgradeMask().testForAny( conflicting ) )
	{
		//If it has ANY of the conflicting OBJECT upgrades, then don't do it!
		return;
	}
	if( getObject()->getControllingPlayer()->getCompletedUpgradeMask().testForAny( conflicting ) )
	{
		//If it has ANY of the conflicting PLAYER upgrades, then don't do it!
		return;
	}

	Object *obj = getObject();
	Drawable *draw = obj->getDrawable();
	if( draw )
	{
		std::vector<AsciiString>::const_iterator subObjectName;
		Bool updateSubObjects = false;

		//Show these subobjects
		for( subObjectName = data->m_showSubObjectNames.begin(); subObjectName != data->m_showSubObjectNames.end(); ++subObjectName )
		{
			draw->showSubObject( *subObjectName, true );
			updateSubObjects = true;
		}

		//Hide these subobjects
		for( subObjectName = data->m_hideSubObjectNames.begin(); subObjectName != data->m_hideSubObjectNames.end(); ++subObjectName )
		{
			draw->showSubObject( *subObjectName, false );
			updateSubObjects = true;
		}

		if( updateSubObjects )
		{
			draw->updateSubObjects();
		}
	}
}

//------------------------------------------------------------------------------------------------
void SubObjectsUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

//------------------------------------------------------------------------------------------------
// Xfer method
// Version Info:
// 1: Initial version 
//------------------------------------------------------------------------------------------------
void SubObjectsUpgrade::xfer( Xfer *xfer )
{

	// version
	XferVersion currentVersion = 1;
	XferVersion version = currentVersion;
	xfer->xferVersion( &version, currentVersion );

	// extend base class
	UpgradeModule::xfer( xfer );

}  // end xfer

//------------------------------------------------------------------------------------------------
void SubObjectsUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
