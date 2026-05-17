import { Link } from 'react-router-dom';

import { Button } from '@/components/ui/button';
import { RouteCard } from '@/components/route-card';

export function NotFoundPage() {
  return (
    <RouteCard
      eyebrow="404"
      title="Route not found"
      description="This placeholder ensures the router has a graceful fallback from the start."
    >
      <Button asChild>
        <Link to="/">Return to the overview</Link>
      </Button>
    </RouteCard>
  );
}
