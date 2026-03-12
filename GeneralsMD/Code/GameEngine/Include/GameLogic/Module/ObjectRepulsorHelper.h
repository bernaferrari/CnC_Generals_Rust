// FILE: ObjectRepulsorHelper.h ////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, December 202
// Desc:   Object helper - Repulsor
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __ObjectRepulsorHelper_H_
#define __ObjectRepulsorHelper_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectRepulsorHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectRepulsorHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ObjectRepulsorHelper, ObjectRepulsorHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ObjectRepulsorHelper, "ObjectRepulsorHelper" )	

public:

	ObjectRepulsorHelper( Thing *thing, const ModuleData *modData ) : ObjectHelper( thing, modData ) { }
	// virtual destructor prototype provided by memory pool object

	virtual UpdateSleepTime update();

};


#endif  // end __ObjectRepulsorHelper_H_
