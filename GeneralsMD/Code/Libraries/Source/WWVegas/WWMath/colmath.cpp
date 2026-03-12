#include "colmath.h"

const float CollisionMath::COINCIDENCE_EPSILON = 0.000001f;
CollisionMath::ColmathStatsStruct CollisionMath::Stats;

CollisionMath::ColmathStatsStruct::ColmathStatsStruct(void)
{
	Reset();
}

void CollisionMath::ColmathStatsStruct::Reset(void)
{
	TotalCollisionCount = 0;
	TotalCollisionHitCount = 0;
	
	CollisionRayTriCount = 0;
	CollisionRayTriHitCount = 0;

	CollisionAABoxTriCount = 0;
	CollisionAABoxTriHitCount = 0;
	CollisionAABoxAABoxCount = 0;
	CollisionAABoxAABoxHitCount = 0;

	CollisionOBBoxTriCount = 0;
	CollisionOBBoxTriHitCount = 0;
	CollisionOBBoxAABoxCount = 0;
	CollisionOBBoxAABoxHitCount = 0;
	CollisionOBBoxOBBoxCount = 0;
	CollisionOBBoxOBBoxHitCount = 0;
}

