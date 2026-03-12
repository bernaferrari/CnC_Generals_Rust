// FILE:  MaxHealthUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Kris Morness, September 2002
// Desc:	 UpgradeModule that increases an object's maximum health.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __MAX_HEALTH_UPGRADE_H_
#define __MAX_HEALTH_UPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
enum MaxHealthChangeType;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class MaxHealthUpgradeModuleData: public UpgradeModuleData
{

public:

	MaxHealthUpgradeModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Real								m_addMaxHealth;
	MaxHealthChangeType m_maxHealthChangeType;

};

//-------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class MaxHealthUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( MaxHealthUpgrade, "MaxHealthUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( MaxHealthUpgrade, MaxHealthUpgradeModuleData );

public:

	MaxHealthUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:

	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};


#endif // __DEFAULTDIE_H_

