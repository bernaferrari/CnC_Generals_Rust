// FILE: RepairDockUpdate.h ///////////////////////////////////////////////////////////////////////
// Author: Colin Day, June 2002
// Desc:   The action of docking with a structure for repairs
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __REPAIRDOCKUPDATE_H_
#define __REPAIRDOCKUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/GameMemory.h"
#include "GameLogic/Module/SupplyCenterDockUpdate.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RepairDockUpdateModuleData : public DockUpdateModuleData
{

public:

	RepairDockUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Real m_framesForFullHeal;			///< time (in frames) something becomes fully repaired

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RepairDockUpdate : public DockUpdate
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RepairDockUpdate, "RepairDockUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( RepairDockUpdate, RepairDockUpdateModuleData )

public:

	RepairDockUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by MemoryPoolObject base class

	virtual DockUpdateInterface* getDockUpdateInterface() { return this; }

	virtual Bool action( Object *docker, Object *drone = NULL );	///< for me this means do some repair

	virtual Bool isRallyPointAfterDockType(){return TRUE;} ///< A minority of docks want to give you a final command to their rally point

protected:

  ObjectID m_lastRepair;			///< object we were repairing last
	Real m_healthToAddPerFrame;	///< health to add per frame to current docked object
	
};

#endif
