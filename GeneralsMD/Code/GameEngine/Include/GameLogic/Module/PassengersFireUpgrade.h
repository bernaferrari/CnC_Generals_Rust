// FILE: PassengersFireUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, May 2003
// Desc:	 UpgradeModule that sets containmodules flag for passengersAllowedToFire
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __PASSENGERS_FIRE_UPGRADE_H_
#define __PASSENGERS_FIRE_UPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

//-------------------------------------------------------------------------------------------------
class PassengersFireUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PassengersFireUpgrade, "PassengersFireUpgrade" )
	MAKE_STANDARD_MODULE_MACRO( PassengersFireUpgrade );

public:

	PassengersFireUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
  virtual Bool isSubObjectsUpgrade() { return false; }


};


#endif // __PASSENGERS_FIRE_UPGRADE_H_

