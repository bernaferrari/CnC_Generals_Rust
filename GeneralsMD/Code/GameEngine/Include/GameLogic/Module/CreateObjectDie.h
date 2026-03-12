// FILE: CreateObjectDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Michael S. Booth, January 2002
// Desc:   Create object at current object's death
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _CREATE_OBJECT_DIE_H_
#define _CREATE_OBJECT_DIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "GameLogic/Module/DieModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;
class ObjectCreationList;

//-------------------------------------------------------------------------------------------------
class CreateObjectDieModuleData : public DieModuleData
{

public:

	const ObjectCreationList* m_ocl;			///< object creaton list to make
	Bool m_transferPreviousHealth; ///< Transfers previous health before death to the new object created.

	CreateObjectDieModuleData();

	static void buildFieldParse(MultiIniFieldParse& p);

};

//-------------------------------------------------------------------------------------------------
/** When this object dies, create another object in its place */
//-------------------------------------------------------------------------------------------------
class CreateObjectDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CreateObjectDie, "CreateObjectDie"  )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CreateObjectDie, CreateObjectDieModuleData );

public:

	CreateObjectDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

};

#endif // _CREATE_OBJECT_DIE_H_

