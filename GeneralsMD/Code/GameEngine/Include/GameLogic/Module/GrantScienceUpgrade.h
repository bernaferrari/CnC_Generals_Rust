// FILE: GrantScienceUpgrade.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Los Angeles
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2003 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	  August 2, 2003
//
//	Filename: 	GrantScienceUpgrade.cpp
//
//	Author:		  Kris Morness
//	
//	Purpose:	  Grants specified science once requirements met (typically an upgrade).
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __GRANT_SCIENCE_UPGRADE_H
#define __GRANT_SCIENCE_UPGRADE_H

#include "GameLogic/Module/UpgradeModule.h"

//-----------------------------------------------------------------------------
class GrantScienceUpgradeModuleData : public UpgradeModuleData
{
public:
	AsciiString m_grantScienceName;

	GrantScienceUpgradeModuleData(){}

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-----------------------------------------------------------------------------
class GrantScienceUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( GrantScienceUpgrade, "GrantScienceUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( GrantScienceUpgrade, GrantScienceUpgradeModuleData );

public:

	GrantScienceUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

private:
	
	ScienceType m_scienceType;

};
#endif // _COMMAND_SET_UPGRADE_H


