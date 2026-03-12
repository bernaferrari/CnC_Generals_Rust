// FILE: CreateCrateDie.h /////////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, February 2002
// Desc:   A chance to create a crate on death according to certain condition checks
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef _CREATE_CRATE_DIE_H_
#define _CREATE_CRATE_DIE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/INI.h"
#include "GameLogic/Module/DieModule.h"
#include "Common/STLTypedefs.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class CrateTemplate;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class CreateCrateDieModuleData : public DieModuleData
{
public:
	AsciiStringList m_crateNameList;

	CreateCrateDieModuleData()
	{
		// Added By Sadullah Nader
		// Initializations missing and needed
		m_crateNameList.clear();
	}
	~CreateCrateDieModuleData()
	{
		m_crateNameList.clear();
	}

	static void buildFieldParse(MultiIniFieldParse& p) 
	{
    DieModuleData::buildFieldParse(p);

		static const FieldParse dataFieldParse[] = 
		{
			{ "CrateData",	CreateCrateDieModuleData::parseCrateData,		NULL, NULL },
			{ 0, 0, 0, 0 }
		};
    p.add(dataFieldParse);
	}

	static void parseCrateData( INI* ini, void *instance, void * /*store*/, const void* /*userData*/ );
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class CreateCrateDie : public DieModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( CreateCrateDie, "CreateCrateDie" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( CreateCrateDie, CreateCrateDieModuleData )

public:

	CreateCrateDie( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	virtual void onDie( const DamageInfo *damageInfo ); 

private:
	Bool testCreationChance( CrateTemplate const *currentCrateData );
	Bool testVeterancyLevel( CrateTemplate const *currentCrateData );
	Bool testKillerType( CrateTemplate const *currentCrateData, Object *killer );
	Bool testKillerScience( CrateTemplate const *currentCrateData, Object *killer );

	Object *createCrate( CrateTemplate const *currentCrateData );
};

#endif 
