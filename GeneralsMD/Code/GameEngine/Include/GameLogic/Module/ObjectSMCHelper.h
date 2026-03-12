// FILE: ObjectSMCHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Colin Day - September 202
// Desc:   Object helpder - SMC
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __OBJECT_SMC_HELPER_H_
#define __OBJECT_SMC_HELPER_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectSMCHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectSMCHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ObjectSMCHelper, ObjectSMCHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ObjectSMCHelper, "ObjectSMCHelperPool" )	

public:

	ObjectSMCHelper( Thing *thing, const ModuleData *modData ) : ObjectHelper( thing, modData ) { }
	// virtual destructor prototype provided by memory pool object

	virtual UpdateSleepTime update();

};


#endif  // end __OBJECT_SMC_HELPER_H_
