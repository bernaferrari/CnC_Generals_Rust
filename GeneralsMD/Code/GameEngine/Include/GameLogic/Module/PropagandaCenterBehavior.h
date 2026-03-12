// FILE: PropagandaCenterBehavior.h ///////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc:   Propaganda Center Behavior
///////////////////////////////////////////////////////////////////////////////////////////////////

#ifndef __PROPAGANDA_CENTER_BEHAVIOR_H_
#define __PROPAGANDA_CENTER_BEHAVIOR_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/PrisonBehavior.h"

#ifdef ALLOW_SURRENDER

// ------------------------------------------------------------------------------------------------
typedef std::list< ObjectID > BrainwashedIDList;
typedef BrainwashedIDList::const_iterator BrainwashedIDListContIterator;
typedef BrainwashedIDList::iterator BrainwashedIDListIterator;

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PropagandaCenterBehaviorModuleData : public PrisonBehaviorModuleData
{

public:

	PropagandaCenterBehaviorModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	UnsignedInt m_brainwashDuration;			///< how long (in frames) it takes to become brainwashed

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class PropagandaCenterBehavior : public PrisonBehavior
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( PropagandaCenterBehavior, "PropagandaCenterBehavior" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( PropagandaCenterBehavior, PropagandaCenterBehaviorModuleData )

public:

	PropagandaCenterBehavior( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool object

	// generic module methods
	virtual void onDelete( void );

	// contain methods
	virtual UpdateSleepTime update( void );
	virtual void onRemoving( Object *obj );

protected:

	ObjectID m_brainwashingSubjectID;								///< who we're currently brainwashing
	UnsignedInt m_brainwashingSubjectStartFrame;		///< frame we started brainwashing
	BrainwashedIDList m_brainwashedList;						///< list of objects we've brainwashed
		
};

#endif

#endif  // end __PROPAGANDA_CENTER_BEHAVIOR_H_
