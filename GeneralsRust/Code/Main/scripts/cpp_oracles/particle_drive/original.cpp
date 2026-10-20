#include <cmath>
#include <cstdlib>
#include <cstdint>
#include <cstring>
#include <iostream>
#include <string>
using Real=float; using Bool=bool; using Int=int; using UnsignedInt=unsigned;
constexpr int LOGICFRAMES_PER_SECOND=30;
struct Coord3D
{
	Real x, y, z;

	Real length( void ) const { return (Real)sqrt( x*x + y*y + z*z ); }
	Real lengthSqr( void ) const { return ( x*x + y*y + z*z ); }

	void normalize( void )
	{
		Real len = length();

		if( len != 0 )
		{
			x /= len;
			y /= len;
			z /= len;
		}
	}

	static void crossProduct( const Coord3D *a, const Coord3D *b, Coord3D *r )
	{
		r->x = (a->y * b->z - a->z * b->y);
		r->y = (a->z * b->x - a->x * b->z);
		r->z = (a->x * b->y - a->y * b->x);
	}

	void zero( void )
	{
		x = 0.0f;
		y = 0.0f;
		z = 0.0f;
	}

	void add( const Coord3D *a )
	{
		x += a->x;
		y += a->y;
		z += a->z;
	}

	void sub( const Coord3D *a )
	{
		x -= a->x;
		y -= a->y;
		z -= a->z;
	}

	void set( const Coord3D *a )
	{
		x = a->x;
		y = a->y;
		z = a->z;
	}

	void set( Real ax, Real ay, Real az )
	{
		x = ax;
		y = ay;
		z = az;
	}

	void scale( Real scale )
	{
		x *= scale;
		y *= scale;
		z *= scale;
	}

	Bool equals( const Coord3D &r )
	{
		return (x == r.x &&
						y == r.y &&
						z == r.z);
	}

	Bool operator==( const Coord3D &r )
	{
		return (x == r.x &&
						y == r.y &&
						z == r.z);
	}
};


struct Waypoint { unsigned id; Coord3D position; Waypoint* next=nullptr;
 unsigned getID(){return id;} int getNumLinks(){return next?1:0;}
 Waypoint* getLink(int i){return i==0?next:nullptr;} const Coord3D* getLocation(){return &position;}
};
struct Terrain { Real height; Waypoint current,next;
 Waypoint* getWaypointByID(unsigned id){return id==current.id?&current:nullptr;}
 Real getGroundHeight(Real,Real){return height;}
};
Terrain* TheTerrainLogic;
Int GameLogicRandomValue(Int lo,Int hi){if(lo!=0||hi!=0) std::abort();return lo;}
struct Data {Real m_manualDrivingSpeed=20,m_manualFastDrivingSpeed=40;unsigned m_doubleClickToFastDriveDelay=15;};
uint32_t bits(float x){uint32_t r;std::memcpy(&r,&x,4);return r;}
int main(){
 std::string tag;float cx,cy,cz,dx,dy,dz,ground,nx,ny,nz;int scripted;
 while(std::cin>>tag>>cx>>cy>>cz>>dx>>dy>>dz>>ground>>scripted>>nx>>ny>>nz){
  Data store;auto* data=&store;Bool m_scriptedWaypointMode=scripted!=0;
  UnsignedInt m_lastDrivingClickFrame=100,m_2ndLastDrivingClickFrame=0,m_nextDestWaypointID=scripted?2:0;
  Coord3D m_currentTargetPosition{cx,cy,cz},m_overrideTargetDestination{dx,dy,dz};
  Terrain terrain;terrain.height=ground;terrain.current={2,{dx,dy,dz}};terrain.next={3,{nx,ny,nz}};terrain.current.next=&terrain.next;TheTerrainLogic=&terrain;
				Real speed = data->m_manualDrivingSpeed;
				if( m_scriptedWaypointMode || m_lastDrivingClickFrame - m_2ndLastDrivingClickFrame < data->m_doubleClickToFastDriveDelay )
				{
					//Because we double clicked, use the faster driving speed.
					speed = data->m_manualFastDrivingSpeed;
				}

				//Convert speed to speed per frame.
				speed /= LOGICFRAMES_PER_SECOND;

				//Calculate the distance from our current position to our target dest.
				Coord3D vector = m_overrideTargetDestination;
				vector.sub( &m_currentTargetPosition );
				Real distance = vector.length();
				if( distance < speed )
				{
					//Don't allow the speed to overshoot the target point if close.
					speed = distance;

					//If we're in scripted waypoint mode, go to the next waypoint!
					if( m_scriptedWaypointMode )
					{
						Waypoint *way = TheTerrainLogic->getWaypointByID( m_nextDestWaypointID );
						if( way )
						{
							//Advance to the next waypoint.
							Int linkCount = way->getNumLinks();
							Int which = GameLogicRandomValue( 0, linkCount-1 );
							way = way->getLink( which );
							m_nextDestWaypointID = way->getID();
							m_overrideTargetDestination.set( way->getLocation() );
						}
					}
				}

				//Unitize the vector then apply the distance we will move.
				vector.normalize();
				vector.scale( speed );

				//Move the current target position in the desired direction and speed.
				m_currentTargetPosition.x += vector.x;
				m_currentTargetPosition.y += vector.y;
			m_currentTargetPosition.z = TheTerrainLogic->getGroundHeight( m_currentTargetPosition.x, m_currentTargetPosition.y );

  std::cout<<tag<<" "<<bits(m_currentTargetPosition.x)<<" "<<bits(m_currentTargetPosition.y)<<" "<<bits(m_currentTargetPosition.z)<<" "<<m_nextDestWaypointID<<"\n";
 }
}
