// FILE: RailedTransportDockUpdate.h //////////////////////////////////////////////////////////////
// Author: Colin Day, August 2002
// Desc:   Railed transport dock update
///////////////////////////////////////////////////////////////////////////////////////////////////

#ifndef __RAILED_TRANSPORT_DOCK_UPDATE_H_
#define __RAILED_TRANSPORT_DOCK_UPDATE_H_

// INCLUDES ///////////////////////////////////////////////////////////////////////////////////////
#include "GameLogic/Module/DockUpdate.h"

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RailedTransportDockUpdateModuleData : public DockUpdateModuleData
{

public:

	RailedTransportDockUpdateModuleData( void );

	static void buildFieldParse( MultiIniFieldParse &p );

	UnsignedInt m_pullInsideDurationInFrames;		/**< how long it takes to pull object inside 
																									 once they're at the dock action point */
	UnsignedInt m_pushOutsideDurationInFrames;	/**< how long it takes to push object outside
																									 when we're unloading */

	Real m_toleranceDistance;	///< The maximum distance the docking unit must be within in order to cheat and dock.
};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RailedTransportDockUpdateInterface
{

public:

	virtual Bool isLoadingOrUnloading( void ) = 0;
	virtual void unloadAll( void ) = 0;
	virtual void unloadSingleObject( Object *obj ) = 0;

};

// ------------------------------------------------------------------------------------------------
// ------------------------------------------------------------------------------------------------
class RailedTransportDockUpdate : public DockUpdate,
																	public RailedTransportDockUpdateInterface
{

	MEMORY_POOL_GLUE_WITH_USERLOOKUP_CREATE( RailedTransportDockUpdate, "RailedTransportDockUpdate" )
	MAKE_STANDARD_MODULE_MACRO_WITH_MODULE_DATA( RailedTransportDockUpdate, RailedTransportDockUpdateModuleData )

public:

	RailedTransportDockUpdate( Thing *thing, const ModuleData *moduleData );
	// virtual destructor prototype provided by memory pool declaration

	// module interfaces
	virtual RailedTransportDockUpdateInterface *getRailedTransportDockUpdateInterface( void ) { return this; }

	// update module methods
	virtual UpdateSleepTime update( void );

	// dock methods
	virtual DockUpdateInterface* getDockUpdateInterface() { return this; }
	virtual Bool action( Object* docker, Object *drone = NULL );
	virtual Bool isClearToEnter( Object const* docker ) const;

	// our own methods
	virtual Bool isLoadingOrUnloading( void );
	virtual void unloadAll( void );
	virtual void unloadSingleObject( Object *obj );

protected:

	void doPullInDocking( void );							///< pull docking objects into us
	void doPushOutDocking( void );						///< push unloading objects out of us
	void unloadNext( void );									///< start the "next" object we have inside us coming out

	ObjectID m_dockingObjectID;								///< object docking with us
	Real m_pullInsideDistancePerFrame;				///< when docking, pull object inside this much each frame

	ObjectID m_unloadingObjectID;							///< object that is currently unloading
	Real m_pushOutsideDistancePerFrame;				///< when unloading, push object outside this much frame

	Int m_unloadCount;												///< count used to govern unloading 1 or all objects

};

#endif  // end __RAILED_TRANSPORT_DOCK_UPDATE_H_
