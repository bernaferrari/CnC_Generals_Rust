// FILE: PrisonBehavior.h /////////////////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc:   Prison Behaviors
///////////////////////////////////////////////////////////////////////////////////////////////////

#ifndef __PRISON_BEHAVIOR_H_
#define __PRISON_BEHAVIOR_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/OpenContain.h"

#ifdef ALLOW_SURRENDER

// FORWARD REFERENCES /////////////////////////////////////////////////////////////////////////////
class PrisonVisual;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PrisonBehaviorModuleData : public OpenContainModuleData
{

public:

	PrisonBehaviorModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	Bool m_showPrisoners;								///< Will the contained prisoners be shown in the prison yard
	AsciiString m_prisonYardBonePrefix;	///< Bone prefix that makes up the prison yards

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PrisonBehavior : public OpenContain
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PrisonBehavior, "PrisonBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PrisonBehavior, PrisonBehaviorModuleData )

public:

	PrisonBehavior( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool object

	// module methods
	virtual void onDelete( void );

	// contain methods
	virtual void onContaining( Object *obj, Bool wasSelected );
	virtual void onRemoving( Object *obj );

protected:

	void pickVisualLocation( Coord3D *pos );		///< pick a location inside the prison yard
	void addVisual( Object *obj );							///< add prisoner visual
	void removeVisual( Object *obj );						///< remove prisoner visual

	PrisonVisual *m_visualList;									///< list of visual representation data (if needed)
			
};

#endif

#endif  // end __PRISON_BEHAVIOR_H_
