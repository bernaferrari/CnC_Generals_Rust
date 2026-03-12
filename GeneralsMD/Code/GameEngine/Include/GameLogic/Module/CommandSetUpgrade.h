// FILE: CommandSetUpgrade.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, September 2002
// Desc:	 UpgradeModule that sets a new override string for Command Set look ups
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _COMMAND_SET_UPGRADE_H
#define _COMMAND_SET_UPGRADE_H

#include "GameLogic/Module/UpgradeModule.h"

//-----------------------------------------------------------------------------
class CommandSetUpgradeModuleData : public UpgradeModuleData
{
public:
	AsciiString m_newCommandSet;
	AsciiString m_newCommandSetAlt;
	AsciiString m_triggerAlt;

	CommandSetUpgradeModuleData()
	{
		m_newCommandSet			= AsciiString::TheEmptyString;
		m_newCommandSetAlt	= AsciiString::TheEmptyString;
		m_triggerAlt				= "none";
	}

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-----------------------------------------------------------------------------
class CommandSetUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CommandSetUpgrade, "CommandSetUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CommandSetUpgrade, CommandSetUpgradeModuleData );

public:

	CommandSetUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};
#endif // _COMMAND_SET_UPGRADE_H


