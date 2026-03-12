#ifndef RENDEROBJECTRECYCLER_H
#define RENDEROBJECTRECYCLER_H

#include "always.h"
#include "robjlist.h"

class RenderObjClass;
class Matrix3D;

/**
** RenderObjectRecyclerClass
** This class can be used to eliminate dynamic render object allocation.  An example usage would be 
** the case where you have a projectile system in a game.  Projectiles could be set up to use a
** variety of render objects and would be rapidly created and destroyed as the players and AI's fire
** their weapons.  If all of your bullet objects request their models from a RenderObjectCache, and
** return their models to the cache when the bullet is destroyed; the model will get re-used by 
** the next bullet that requests that same model type (highly likely in a game situation).  
**
** Public member functions:
** Reset - release all of the models in the cache.
** Get_Render_Object - returns either recycles a model or creates a new one through the asset manager
** Return_Render_Object - give your model back to the recycler for re-use later.
*/
class RenderObjectRecyclerClass
{
public:

	void					Reset(void);
	RenderObjClass*	Get_Render_Object(const char * name,const Matrix3D & tm);
	void					Return_Render_Object(RenderObjClass * obj);
	
private:

	void					Insert_Inactive_Model(RenderObjClass * obj);
	void					Reset_Model(RenderObjClass * model);


	RefRenderObjListClass	InactiveModels;

};




#endif //RENDEROBJECTRECYCLER_H