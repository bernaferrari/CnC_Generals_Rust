// FILE: ExperienceScalarUpgrade.cpp /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, September 2002
// Desc:	 UpgradeModule that adds a scalar to the object's experience gain.
///////////////////////////////////////////////////////////////////////////////////////////////////

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "PreRTS.h"	// This must go first in EVERY cpp file int the GameEngine

#include "Common/Xfer.h"
#include "GameLogic/Object.h"
#include "GameLogic/ExperienceTracker.h"
#include "GameLogic/Module/ExperienceScalarUpgrade.h"

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ExperienceScalarUpgradeModuleData::ExperienceScalarUpgradeModuleData( void )
{
	m_addXPScalar = 0.0f;
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ExperienceScalarUpgradeModuleData::buildFieldParse(MultiIniFieldParse& p) 
{

  UpgradeModuleData::buildFieldParse( p );

	static const FieldParse dataFieldParse[] = 
	{
		{ "AddXPScalar",	INI::parseReal,		NULL, offsetof( ExperienceScalarUpgradeModuleData, m_addXPScalar ) },
		{ 0, 0, 0, 0 }
	};

  p.add(dataFieldParse);

}  // end buildFieldParse

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ExperienceScalarUpgrade::ExperienceScalarUpgrade( Thing *thing, const ModuleData* moduleData ) : UpgradeModule( thing, moduleData )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
ExperienceScalarUpgrade::~ExperienceScalarUpgrade( void )
{
}

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
void ExperienceScalarUpgrade::upgradeImplementation( )
{
	const ExperienceScalarUpgradeModuleData *data = getExperienceScalarUpgradeModuleData();

	//Simply add the xp scalar to the xp tracker!
	Object *obj = getObject();
	ExperienceTracker *xpTracker = obj->getExperienceTracker();
	if( xpTracker )
	{
		xpTracker->setExperienceScalar( xpTracker->getExperienceScalar() + data->m_addXPScalar );
	}
}

// ------------------------------------------------------------------------------------------------
/** CRC */
// ------------------------------------------------------------------------------------------------
void ExperienceScalarUpgrade::crc( Xfer *xfer )
{

	// extend base class
	UpgradeModule::crc( xfer );

}  // end crc

// ------------------------------------------------------------------------------------------------
/** Xfer method
	* Version Info:
	* 1: Initial version */
// ------------------------------------------------------------------------------------------------
void ExperienceScalarUpgrade::xfer( Xfer *xfer )
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
void ExperienceScalarUpgrade::loadPostProcess( void )
{

	// extend base class
	UpgradeModule::loadPostProcess();

}  // end loadPostProcess
