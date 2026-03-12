// FILE: SubObjectsUpgrade.h /////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//                                                                          
//                       Electronic Arts Pacific.                          
//                                                                          
//                       Confidential Information                           
//                Copyright (C) 2002 - All Rights Reserved                  
//                                                                          
//-----------------------------------------------------------------------------
//
//	Created:	September 2002
//
//	Filename: SubObjectsUpgrade.cpp
//
//	Author:		Kris Morness
//	
//	Purpose:	Shows or hides a list of subobjects based on upgrade statii. It
//            will override any animation subobjects states.
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __SUB_OBJECTS_UPGRADE_H
#define __SUB_OBJECTS_UPGRADE_H

//-----------------------------------------------------------------------------
// USER INCLUDES //////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
#include "GameLogic/Module/UpgradeModule.h"

//-----------------------------------------------------------------------------
// FORWARD REFERENCES /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
class Thing;

//-----------------------------------------------------------------------------
class SubObjectsUpgradeModuleData : public UpgradeModuleData
{
public:
	std::vector<AsciiString>					m_showSubObjectNames;
	std::vector<AsciiString>					m_hideSubObjectNames;

	SubObjectsUpgradeModuleData(){}

	static void buildFieldParse(MultiIniFieldParse& p);
};

//-----------------------------------------------------------------------------
class SubObjectsUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( SubObjectsUpgrade, "SubObjectsUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( SubObjectsUpgrade, SubObjectsUpgradeModuleData );

public:

	SubObjectsUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

protected:
	virtual void upgradeImplementation( ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return true; }

};

#endif


