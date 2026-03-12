// FILE: UnpauseSpecialPowerUpgrade.h ///////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:	 An upgrade that starts the timer on a Special Power module, so you can have them 
// dependent on upgrades on the logic side, like NEED_UPGRADE does on the client side by disabling
// the button.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __UNPAUSE_SPECIAL_POWER_UPGRADE_H_
#define __UNPAUSE_SPECIAL_POWER_UPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class SpecialPowerTemplate;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class UnpauseSpecialPowerUpgradeModuleData : public UpgradeModuleData
{

public:

	UnpauseSpecialPowerUpgradeModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	const SpecialPowerTemplate *m_specialPower;

};

//-------------------------------------------------------------------------------------------------
/** The OCL upgrade module */
//-------------------------------------------------------------------------------------------------
class UnpauseSpecialPowerUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( UnpauseSpecialPowerUpgrade, "UnpauseSpecialPowerUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( UnpauseSpecialPowerUpgrade, UnpauseSpecialPowerUpgradeModuleData );

public:

	UnpauseSpecialPowerUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:

	virtual void upgradeImplementation( void ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};

#endif 

