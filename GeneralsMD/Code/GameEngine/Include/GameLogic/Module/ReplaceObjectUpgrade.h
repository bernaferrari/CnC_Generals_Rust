// FILE: ReplaceObjectUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, July 2003
// Desc:	 UpgradeModule that creates a new Object in our exact location and then deletes our object
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _REPLACE_OBJECT_UPGRADE_H
#define _REPLACE_OBJECT_UPGRADE_H

#include "GameLogic/Module/UpgradeModule.h"

//-----------------------------------------------------------------------------
class ReplaceObjectUpgradeModuleData : public UpgradeModuleData
{
public:
	AsciiString m_replaceObjectName;

	ReplaceObjectUpgradeModuleData()
	{
	}

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-----------------------------------------------------------------------------
class ReplaceObjectUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ReplaceObjectUpgrade, "ReplaceObjectUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ReplaceObjectUpgrade, ReplaceObjectUpgradeModuleData );

public:

	ReplaceObjectUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};
#endif // _COMMAND_SET_UPGRADE_H


