// FILE: ObjectDefectionHelper.h //////////////////////////////////////////////////////////////////
// Author: Steven Johnson, Colin Day - September 202
// Desc:   Object helpder - defection
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __OBJECT_DEFECTION_HELPER_H_
#define __OBJECT_DEFECTION_HELPER_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/ObjectHelper.h"

//-------------------------------------------------------------------------------------------------
#define DEFECTION_DETECTION_TIME_MAX (LOGICFRAMES_PER_SECOND * 10)

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectDefectionHelperModuleData : public ModuleData
{

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class ObjectDefectionHelper : public ObjectHelper
{

	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( ObjectDefectionHelper, ObjectDefectionHelperModuleData )
	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE(ObjectDefectionHelper, "ObjectDefectionHelperPool" )	

private:

	UnsignedInt   m_defectionDetectionStart;				///< this is the timer, mentioned above (absolute frame, NOT counter)
	UnsignedInt   m_defectionDetectionEnd;					///< this is the timer, mentioned above (absolute frame, NOT counter)
	Real          m_defectionDetectionFlashPhase;   ///< keeps track of the flashing rate logarithmic curve
	Bool					m_doDefectorFX;	///<AmericaInfPilot uses defect to become temporarily "invulnerable"

public:

	ObjectDefectionHelper( Thing *thing, const ModuleData *modData ) : ObjectHelper( thing, modData ) 
	{
		//Added By Sadullah Nader
		//Initializations inserted
		m_defectionDetectionEnd = 0;
		m_defectionDetectionFlashPhase = FALSE;
		m_defectionDetectionStart = 0;
		m_doDefectorFX = FALSE;
		//
	}
	// virtual destructor prototype provided by memory pool object

	virtual UpdateSleepTime update();

	// Disabled conditions to process -- defection helper must ignore all disabled types.
	virtual DisabledMaskType getDisabledTypesToProcess() const { return DISABLEDMASK_ALL; }

	// specific to this class.
	void startDefectionTimer(UnsignedInt numFrames, Bool withDefectorFX = TRUE);

};


#endif  // end __OBJECT_DEFECTION_HELPER_H_
