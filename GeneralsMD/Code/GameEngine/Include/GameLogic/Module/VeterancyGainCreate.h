// FILE: VeterancyGainCreate.h //////////////////////////////////////////////////////////////////////////
// Author: Graham Smallwood, August 2002
// Desc:   On creation, will set Object's veterancy level if required Science is present.
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __VETERANCY_GAIN_CREATE_H_
#define __VETERANCY_GAIN_CREATE_H_

#define DEFINE_VETERANCY_NAMES
// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "Common/GameCommon.h"
#include "Common/Science.h"
#include "GameLogic/Module/CreateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class Thing;

class VeterancyGainCreateModuleData : public CreateModuleData
{
public:
	VeterancyLevel m_startingLevel;			///< Level to set Object at
	ScienceType m_scienceRequired;			///< The science you must have to trigger this

	VeterancyGainCreateModuleData();
	static void buildFieldParse(MultiIniFieldParse& p);
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class VeterancyGainCreate : public CreateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( VeterancyGainCreate, "VeterancyGainCreate" );
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( VeterancyGainCreate, VeterancyGainCreateModuleData );

public:

	VeterancyGainCreate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	/// the create method
	virtual void onCreate( void );

protected:

};

#endif 

