// FILE: ActiveShroudUpgrade.h ///////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, July 2002
// Desc:	 An upgrade that modifies the object's ShroudRange.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ACTIVE_SHROUD_UPGRADE_H_
#define __ACTIVE_SHROUD_UPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class Player;
class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class ActiveShroudUpgradeModuleData : public UpgradeModuleData
{

public:

	ActiveShroudUpgradeModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Real m_newShroudRange;

};

//-------------------------------------------------------------------------------------------------
/** An upgrade that modifies the object's ShroudRange */
//-------------------------------------------------------------------------------------------------
class ActiveShroudUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ActiveShroudUpgrade, "ActiveShroudUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ActiveShroudUpgrade, ActiveShroudUpgradeModuleData );

public:

	ActiveShroudUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:

	virtual void upgradeImplementation( void ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};

#endif // __ACTIVE_SHROUD_UPGRADE_H_

