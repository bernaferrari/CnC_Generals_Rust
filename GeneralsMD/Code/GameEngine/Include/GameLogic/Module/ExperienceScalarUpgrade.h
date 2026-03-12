// FILE: ExperienceScalarUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, September 2002
// Desc:	 UpgradeModule that adds a scalar to the object's experience gain.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __EXPERIENCE_SCALAR_UPGRADE_H_
#define __EXPERIENCE_SCALAR_UPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ExperienceScalarUpgradeModuleData: public UpgradeModuleData
{

public:

	ExperienceScalarUpgradeModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Real m_addXPScalar;

};

//-------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ExperienceScalarUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ExperienceScalarUpgrade, "ExperienceScalarUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ExperienceScalarUpgrade, ExperienceScalarUpgradeModuleData );

public:

	ExperienceScalarUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:

	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};


#endif // __DEFAULTDIE_H_

