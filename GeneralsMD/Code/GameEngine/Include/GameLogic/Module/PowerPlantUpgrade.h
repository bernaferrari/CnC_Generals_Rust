// FILE: PowerPlantUpgrade.h ///////////////////////////////////////////////////////////////////////////
// Author: Amit Kumar, August 2002
// Desc:	 Upgrading a power plant
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __POWERPLANTUPGRADE_H_
#define __POWERPLANTUPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class Player;

//-------------------------------------------------------------------------------------------------
/** The Power Plant upgrade module */
//-------------------------------------------------------------------------------------------------
class PowerPlantUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PowerPlantUpgrade, "PowerPlantUpgrade" )
	MAKE_STANDARD_MODULE_MACRO( PowerPlantUpgrade );

public:

	PowerPlantUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	virtual void onDelete( void );																///< we have some work to do when this module goes away
	virtual void onCapture( Player *oldOwner, Player *newOwner );	///< object containing upgrade has changed teams

protected:

	virtual void upgradeImplementation( void ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};

#endif // __POWERPLANTUPGRADE_H_

