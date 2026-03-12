// FILE: ObjectCreationUpgrade.h ///////////////////////////////////////////////////////////////////////////
// Author: Matthew D. Campbell, April 2002
// Desc:	 upgrades that spawn OCLs
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __OBJECTCREATIONUPGRADE_H_
#define __OBJECTCREATIONUPGRADE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpgradeModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class Player;
class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
//-------------------------------------------------------------------------------------------------
class ObjectCreationUpgradeModuleData : public UpgradeModuleData
{

public:

	ObjectCreationUpgradeModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	const ObjectCreationList *m_ocl;			///< the object creation list to make

};

//-------------------------------------------------------------------------------------------------
/** The OCL upgrade module */
//-------------------------------------------------------------------------------------------------
class ObjectCreationUpgrade : public UpgradeModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( ObjectCreationUpgrade, "ObjectCreationUpgrade" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ObjectCreationUpgrade, ObjectCreationUpgradeModuleData );

public:

	ObjectCreationUpgrade( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype defined by MemoryPoolObject

	void onDelete( void );																///< we have some work to do when this module goes away

protected:

	virtual void upgradeImplementation( void ); ///< Here's the actual work of Upgrading
	virtual Bool isSubObjectsUpgrade() { return false; }

};

#endif // __OBJECTCREATIONUPGRADE_H_

