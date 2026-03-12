// FILE: TensileFormationUpdate.h ////////////////////////////////////////////////////////////////////////////
// Author: Mark Lorenzen, November 2002
// Desc:   Springy formation movement like that of say, an avalanche
///////////////////////////////////////////////////////////////////////////////////////////////////

#pragma once

#ifndef __TENSILEFORMATIONUPDATE_H_
#define __TENSILEFORMATIONUPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/UpdateModule.h"

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
struct FieldParse;

//-------------------------------------------------------------------------------------------------
class TensileFormationUpdateModuleData: public UpdateModuleData
{

public:

	TensileFormationUpdateModuleData( void );

	static void buildFieldParse(MultiIniFieldParse& p);

	Bool m_enabled;							///< enabled
	AudioEventRTS				m_crackSound;						

};

//-------------------------------------------------------------------------------------------------
class TensileFormationUpdate : public UpdateModule
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( TensileFormationUpdate, "TensileFormationUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( TensileFormationUpdate, TensileFormationUpdateModuleData )

public:

	TensileFormationUpdate( Thing *thing, const ModuleData* moduleData );
	// virtual destructor prototype provided by memory pool declaration

	void setEnabled( Bool enabled ) { m_enabled = enabled; }  ///< enable/disable formation
	virtual UpdateSleepTime update();	///< Deciding whether or not to make new guys

protected:

	void propagateDislodgement( Bool enabled );
	void initLinks( void );

	struct TensileLink
	{
		ObjectID id;
		Coord3D tensor;
	};

	TensileLink m_links[4];//standard textile algorithm
	Coord3D m_inertia;
	Bool m_enabled;			///< enabled
	Bool m_linksInited;
	UnsignedInt m_motionlessCounter; 
	UnsignedInt m_life;
	Real m_lowestSlideElevation;

	AudioEventRTS				m_crackSound;						

};

#endif  // end __TENSILEFORMATIONUPDATE_H_
