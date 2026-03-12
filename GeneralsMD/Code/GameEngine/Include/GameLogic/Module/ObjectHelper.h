// FILE: ObjectHelper.h ///////////////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Colin Day - September 202
// Desc:   Object helpder
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __OBJECT_HELPER_H_
#define __OBJECT_HELPER_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectHelper : public UpdateModule
{

	MEMORY_POOL_GLUE_ABC( ObjectHelper )

protected:

	// snapshot methods
	virtual void crc( Xfer *xfer );
	virtual void xfer( Xfer *xfer );
	virtual void loadPostProcess( void );

public:

	ObjectHelper( Thing *thing, const ModuleData *modData ) : 
		UpdateModule( thing, modData ) 
	{ 
		setWakeFrame(getObject(), UPDATE_SLEEP_FOREVER);
	}

	// inherited from UpdateModuleInterface
	virtual UpdateSleepTime update() = 0;

	// custom to this class.
	void sleepUntil(UnsignedInt when);

};

#endif  // end __OBJECT_HELPER_H_
