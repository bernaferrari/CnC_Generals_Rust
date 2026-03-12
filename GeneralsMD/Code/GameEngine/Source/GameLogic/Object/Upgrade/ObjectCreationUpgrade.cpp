// FILE: ObjectCreationUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, April 2002
// Desc:	 upgrades that spawn OCLs
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/ModelState.h"
#include "Common/Player.h"
#include "Common/Xfer.h"
#include "GameClient/Drawable.h"
#include "GameLogic/Module/ObjectCreationUpgrade.h"
#include "GameLogic/Object.h"
#include "GameLogic/ObjectCreationList.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ObjectCreationUpgradeModuleData::ObjectCreationUpgradeModuleData( void )
{

	m_ocl = NULL;

}  // end SpecialPowerModuleData

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
/* static */ void ObjectCreationUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p)
{
	UpgradeModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "UpgradeObject", INI::parseObjectCreationList, NULL, offsetof( ObjectCreationUpgradeModuleData, m_ocl ) },
		{ 0, 0, 0, 0 } 
	};
	p.add(dataFieldParse);

}  // end buildFieldParse

///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////
///////////////////////////////////////////////////////////////////////////////////////////////////

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ObjectCreationUpgrade::ObjectCreationUpgrade( Thing *thing, const ModuleData* moduleData ) : 
							UpgradeModule( thing, moduleData )
{

}  // end ObjectCreationUpgrade

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ObjectCreationUpgrade::~ObjectCreationUpgrade( void )
{

}  // end ~ObjectCreationUpgrade

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ObjectCreationUpgrade::onDelete( void )
{
}  // end onDelete

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ObjectCreationUpgrade::upgradeImplementation( void )
{
	// spawn everything in the OCL
	if (getObjectCreationUpgradeModuleData() && getObjectCreationUpgradeModuleData()->m_ocl)
	{
		ObjectCreationList::create((getObjectCreationUpgradeModuleData()->m_ocl), getObject(), NULL);
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ObjectCreationUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ObjectCreationUpgrade::xfer( Xfer *xfer )
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
void ObjectCreationUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
