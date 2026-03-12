#ifndef CAMERASHAKESYSTEM_H
#define CAMERASHAKESYSTEM_H


#include "always.h"
#include "vector3.h"
#include "multilist.h"
#include "mempool.h"

class CameraClass;

/**
** CameraShakeSystemClass
** This class encapsulates all of the logic and data needed to implement camera "shakes"
** These are used to simulate explosions, earthquakes, etc.  
*/
class CameraShakeSystemClass
{
public:

	CameraShakeSystemClass(void);
	~CameraShakeSystemClass(void);

	enum 
	{
		FLAGS_NONE = 0,
		FLAGS_IGNOREPOSITION,
	};
	
	void		Add_Camera_Shake(		const Vector3 & position,
											float radius = 50.0f,
											float duration = 1.5f,
											float power = 1.0f	);
	void		Timestep(float dt);
	bool		IsCameraShaking(void);
	void		Update_Camera_Shaker(Vector3 camera_position, Vector3 * shaker_angles);

protected:

	/**
	** CameraShakerClass 
	** This class encapsulates the current state of a camera shaker.  It is a multi-list object
	** and is allocated in pools.
	*/
	class CameraShakerClass : public MultiListObjectClass, public AutoPoolClass<CameraShakerClass,256>
	{	
	public:
		CameraShakerClass(const Vector3 & position,float radius,float duration,float power);
		~CameraShakerClass(void);

		void					Timestep(float dt)							{ ElapsedTime += dt; }	
		bool					Is_Expired(void)								{ return (ElapsedTime >= Duration); }
		void					Compute_Rotations(const Vector3 & pos,Vector3 * set_angles);

	protected:

		Vector3				Position;
		float					Radius;
		float					Duration;
		float					Intensity;
	
		float					ElapsedTime;
		Vector3				Omega;
		Vector3				Phi;
	};

	MultiListClass<CameraShakerClass>	CameraShakerList;

};

extern CameraShakeSystemClass CameraShakerSystem; //WST 11/12/2002 This is the new Camera Shaker system upgrade


#endif //CAMERASHAKESYSTEM_H

